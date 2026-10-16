// ABOUTME: Auto-connect (ADR-085 §4): run an invocation through the daemon serving its repository when one answers,
// ABOUTME: streaming its stderr as it comes; run it here otherwise, with one line on stderr when a daemon failed it.
use std::io::Write;
use std::path::PathBuf;

use cupel_serve::{Body, ClientError, Invocation, Reply, Request};

use crate::app::{Environment, Output, command_words, repository_root, run};
use crate::format;
use crate::serve;

/// Runs `environment`'s invocation through the daemon when one serves its repository and may
/// run it, else in this process.
pub fn invoke(environment: &Environment) -> Output {
    let Some((socket, request)) = daemon_request(environment) else {
        return run(environment);
    };
    let mut stderr = environment.stderr.clone();
    let replied = cupel_serve::request(&socket, &request, &mut |text| {
        let _ = stderr.write_all(text.as_bytes());
        let _ = stderr.flush();
    });
    let why = match replied {
        Ok(Reply::Done(done)) => {
            return Output {
                stdout: done.stdout,
                stderr: done.stderr,
                code: done.code,
            };
        }
        Err(ClientError::Absent | ClientError::Unanswered(_)) => return run(environment),
        Err(error) => error.to_string(),
        Ok(other) => format!("the daemon answered a run with {other:?}"),
    };
    let mut output = run(environment);
    let quiet = format::recover(&environment.args, None, environment.stdout_is_terminal).quiet;
    if !quiet {
        output.stderr = format!("cupel: {why}; ran without the daemon.\n{}", output.stderr);
    }
    output
}

/// The daemon's socket and the request for `environment`, unless the invocation runs here:
/// `CUPEL_NO_DAEMON=1`, the `serve` verb, an argument that is not UTF-8, a run that could ask
/// on the terminal, or no socket file in the repository.
fn daemon_request(environment: &Environment) -> Option<(PathBuf, Request)> {
    if environment.variable("CUPEL_NO_DAEMON") == Some("1") {
        return None;
    }
    let args: Vec<String> = environment
        .args
        .iter()
        .map(|arg| arg.clone().into_string().ok())
        .collect::<Option<_>>()?;
    let flags: Vec<&str> = args
        .iter()
        .map(String::as_str)
        .take_while(|arg| *arg != "--")
        .collect();
    let answers_itself = flags.contains(&"--install") || flags.contains(&"--no-install");
    if environment.stdout_is_terminal && !answers_itself {
        return None;
    }
    if command_words(&environment.args).first().map(String::as_str) == Some("serve") {
        return None;
    }
    let root = repository_root(environment.current_dir.as_deref(), cwd(&flags).as_deref()).ok()?;
    let socket = serve::default_socket(&root);
    if !socket.exists() {
        return None;
    }
    let invocation = Invocation {
        args,
        current_dir: environment.current_dir.clone(),
        stdout_is_terminal: environment.stdout_is_terminal,
        variables: environment.variables.clone(),
        no_color: environment.no_color_variable,
    };
    let body = Body::Run {
        path: environment.path.clone(),
        invocation,
    };
    Some((socket, Request::new(serve::build(), body)))
}

/// The value of `--cwd` among `flags`, as `--cwd DIR` or `--cwd=DIR`.
fn cwd(flags: &[&str]) -> Option<PathBuf> {
    flags.iter().enumerate().find_map(|(index, flag)| {
        if *flag == "--cwd" {
            flags.get(index + 1).map(PathBuf::from)
        } else {
            flag.strip_prefix("--cwd=").map(PathBuf::from)
        }
    })
}
