// ABOUTME: A static read of a Maven module's inheritance chain (its pom.xml, then each parent found on disk or in the local
// ABOUTME: repository) that tells whether the build runs a plugin's goal itself, such as JaCoCo's prepare-agent.
use std::path::{Path, PathBuf};

use roxmltree::{Document, Node};

/// How many POMs of a chain are read at most.
const DEPTH: usize = 16;

/// The groupId a plugin without one has.
const DEFAULT_GROUP: &str = "org.apache.maven.plugins";

/// Whether the build of the `pom.xml` in `dir` runs `goal` of `plugin` (`groupId:artifactId`)
/// in its lifecycle: a POM of its inheritance chain declares the plugin under
/// `<build><plugins>`, and a POM of the chain gives the plugin an execution of the goal, there
/// or under `<pluginManagement>`. Profiles, `<inherited>`, and parents neither on disk nor in
/// the local repository are not read.
pub(crate) fn runs_goal(dir: &Path, plugin: &str, goal: &str) -> bool {
    runs_goal_in(dir, &local_repository(), plugin, goal)
}

/// [`runs_goal`] with the local repository at `repository`.
pub(crate) fn runs_goal_in(dir: &Path, repository: &Path, plugin: &str, goal: &str) -> bool {
    let Some((group, artifact)) = plugin.split_once(':') else {
        return false;
    };
    let texts = chain(&dir.join("pom.xml"), repository);
    let docs: Vec<Document<'_>> = texts
        .iter()
        .filter_map(|text| Document::parse(text).ok())
        .collect();
    let named = |node: &Node<'_, '_>| is_plugin(*node, group, artifact);
    let declared = docs
        .iter()
        .any(|doc| plugins(doc.root_element(), false).any(|node| named(&node)));
    let executed = docs.iter().any(|doc| {
        plugins(doc.root_element(), false)
            .chain(plugins(doc.root_element(), true))
            .filter(|node| named(node))
            .any(|node| has_goal(node, goal))
    });
    declared && executed
}

/// The text of the POM at `pom` and of each parent after it, nearest first.
fn chain(pom: &Path, repository: &Path) -> Vec<String> {
    let mut texts = Vec::new();
    let mut next = Some(pom.to_path_buf());
    while let Some(path) = next.take().filter(|_| texts.len() < DEPTH) {
        let Ok(text) = std::fs::read_to_string(&path) else {
            break;
        };
        next = Document::parse(&text)
            .ok()
            .and_then(|doc| parent(doc.root_element(), &path, repository));
        texts.push(text);
    }
    texts
}

/// Where the parent of the POM at `pom` (whose `<project>` is `project`) is: its
/// `relativePath` (`../pom.xml` by default) when the POM there is that parent, else its file in
/// the local repository; `None` without a parent.
fn parent(project: Node<'_, '_>, pom: &Path, repository: &Path) -> Option<PathBuf> {
    let parent = child(project, "parent")?;
    let (group, artifact, version) = (
        text(parent, "groupId")?,
        text(parent, "artifactId")?,
        text(parent, "version")?,
    );
    let relative = match child(parent, "relativePath") {
        Some(node) => node.text().unwrap_or_default().trim().to_owned(),
        None => "../pom.xml".to_owned(),
    };
    let beside = (!relative.is_empty())
        .then(|| pom.parent().map(|dir| dir.join(&relative)))
        .flatten()
        .map(|path| {
            if path.is_dir() {
                path.join("pom.xml")
            } else {
                path
            }
        })
        .filter(|path| names_artifact(path, artifact));
    beside.or_else(|| {
        let file = repository
            .join(group.replace('.', "/"))
            .join(artifact)
            .join(version)
            .join(format!("{artifact}-{version}.pom"));
        file.is_file().then_some(file)
    })
}

/// Whether the POM at `path` is the one of `artifact`.
fn names_artifact(path: &Path, artifact: &str) -> bool {
    std::fs::read_to_string(path).is_ok_and(|pom| {
        Document::parse(&pom)
            .is_ok_and(|doc| text(doc.root_element(), "artifactId") == Some(artifact))
    })
}

/// The `<plugin>` elements of `project`'s `<build><plugins>`, or of its
/// `<build><pluginManagement><plugins>` when `managed`.
fn plugins<'a, 'input>(
    project: Node<'a, 'input>,
    managed: bool,
) -> impl Iterator<Item = Node<'a, 'input>> {
    let build = child(project, "build");
    let holder = if managed {
        build.and_then(|build| child(build, "pluginManagement"))
    } else {
        build
    };
    holder
        .and_then(|holder| child(holder, "plugins"))
        .into_iter()
        .flat_map(|plugins| {
            plugins
                .children()
                .filter(|node| node.has_tag_name_local("plugin"))
        })
}

/// Whether the `<plugin>` element `node` is `group:artifact`.
fn is_plugin(node: Node<'_, '_>, group: &str, artifact: &str) -> bool {
    text(node, "artifactId") == Some(artifact)
        && text(node, "groupId").unwrap_or(DEFAULT_GROUP) == group
}

/// Whether one of the `<plugin>` element `node`'s executions names `goal`.
fn has_goal(node: Node<'_, '_>, goal: &str) -> bool {
    child(node, "executions").is_some_and(|executions| {
        executions
            .descendants()
            .filter(|node| node.has_tag_name_local("goal"))
            .any(|node| node.text().map(str::trim) == Some(goal))
    })
}

/// The first child element of `node` named `name`.
fn child<'a, 'input>(node: Node<'a, 'input>, name: &str) -> Option<Node<'a, 'input>> {
    node.children().find(|child| child.has_tag_name_local(name))
}

/// The trimmed text of `node`'s child element `name`.
fn text<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    child(node, name)?.text().map(str::trim)
}

/// The local repository Maven uses: `-Dmaven.repo.local=` in `MAVEN_ARGS` or `MAVEN_OPTS`,
/// else `~/.m2/repository`.
fn local_repository() -> PathBuf {
    ["MAVEN_ARGS", "MAVEN_OPTS"]
        .into_iter()
        .filter_map(std::env::var_os)
        .find_map(|value| {
            value
                .to_string_lossy()
                .split_whitespace()
                .find_map(|word| word.strip_prefix("-Dmaven.repo.local="))
                .map(PathBuf::from)
        })
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join(".m2/repository")
        })
}

/// Element names compared without their namespace, as POMs declare Maven's.
trait LocalName {
    fn has_tag_name_local(&self, name: &str) -> bool;
}

impl LocalName for Node<'_, '_> {
    fn has_tag_name_local(&self, name: &str) -> bool {
        self.is_element() && self.tag_name().name() == name
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::runs_goal_in;

    const JACOCO: &str = "org.jacoco:jacoco-maven-plugin";

    /// A POM of `artifact` with `parent` (a `<parent>` element or nothing) and `build`.
    fn pom(artifact: &str, parent: &str, build: &str) -> String {
        format!(
            "<project xmlns=\"http://maven.apache.org/POM/4.0.0\"><modelVersion>4.0.0</modelVersion>\
             <groupId>g</groupId><artifactId>{artifact}</artifactId><version>1</version>{parent}\
             <build>{build}</build></project>"
        )
    }

    /// The `JaCoCo` plugin element, with `executions` inside it.
    fn jacoco(executions: &str) -> String {
        format!(
            "<plugin><groupId>org.jacoco</groupId><artifactId>jacoco-maven-plugin</artifactId>\
             {executions}</plugin>"
        )
    }

    const PREPARE: &str =
        "<executions><execution><goals><goal>prepare-agent</goal></goals></execution></executions>";

    const PARENT: &str =
        "<parent><groupId>g</groupId><artifactId>base</artifactId><version>1</version></parent>";

    /// Whether the module written as `files` (path, text) under a scratch directory runs
    /// prepare-agent, with `repository/` of that directory as the local repository.
    fn runs(files: &[(&str, String)]) -> bool {
        let dir = tempfile::tempdir().unwrap();
        for (path, text) in files {
            let path = dir.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        runs_goal_in(
            &dir.path().join("module"),
            &dir.path().join("repository"),
            JACOCO,
            "prepare-agent",
        )
    }

    fn plugins(plugins: &str) -> String {
        format!("<plugins>{plugins}</plugins>")
    }

    fn managed(plugins: &str) -> String {
        format!("<pluginManagement><plugins>{plugins}</plugins></pluginManagement>")
    }

    #[test]
    fn a_module_runs_a_goal_its_chain_declares_and_executes() {
        let repository = "repository/g/base/1/base-1.pom";
        assert_eq!(
            [
                runs(&[("module/pom.xml", pom("m", "", &plugins(&jacoco(PREPARE))))]),
                runs(&[
                    ("module/pom.xml", pom("m", PARENT, &plugins(&jacoco("")))),
                    ("pom.xml", pom("base", "", &managed(&jacoco(PREPARE)))),
                ]),
                runs(&[
                    ("module/pom.xml", pom("m", PARENT, &plugins(&jacoco("")))),
                    (repository, pom("base", "", &managed(&jacoco(PREPARE)))),
                ]),
                runs(&[
                    ("module/pom.xml", pom("m", PARENT, "")),
                    ("pom.xml", pom("base", "", &plugins(&jacoco(PREPARE)))),
                ]),
            ],
            [true, true, true, true]
        );
    }

    #[test]
    fn a_module_does_not_run_a_goal_only_managed_or_never_executed() {
        assert_eq!(
            [
                runs(&[("module/pom.xml", pom("m", "", &managed(&jacoco(PREPARE))))]),
                runs(&[("module/pom.xml", pom("m", "", &plugins(&jacoco(""))))]),
                runs(&[("module/pom.xml", pom("m", PARENT, &plugins(&jacoco(""))))]),
                runs(&[
                    ("module/pom.xml", pom("m", PARENT, &plugins(&jacoco("")))),
                    ("pom.xml", pom("other", "", &managed(&jacoco(PREPARE)))),
                ]),
                runs(&[("module/pom.xml", "<project>".to_owned())]),
            ],
            [false, false, false, false, false]
        );
    }
}
