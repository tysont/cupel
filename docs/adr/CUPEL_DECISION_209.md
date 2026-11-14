# CUPEL_DECISION_209: A Maven build that runs JaCoCo's agent itself gets its own report goal

**Date:** 2026-11-13 · **Status:** accepted · **Phase:** v0.2.9, item V8 · Details [108](CUPEL_DECISION_108.md) (Java coverage through JaCoCo)

## Context
ADR-108 gave Maven projects JaCoCo by coordinate, `mvn -B --no-transfer-progress org.jacoco:jacoco-maven-plugin:0.8.13:prepare-agent test org.jacoco:jacoco-maven-plugin:0.8.13:report`, so a `pom.xml` needs no change. The v0.2.9 validation on FasterXML/jackson-core (3.x) found a build that already runs the agent: its `pom.xml` declares `jacoco-maven-plugin` under `<build><plugins>` with no version or executions, its parent `tools.jackson:jackson-base` manages the version and a `prepare-agent` execution, and Surefire's `<argLine>` starts with `@{argLine}`. Plain `mvn test` passes and writes `target/jacoco.exec`. Cupel's command adds a second `prepare-agent`; with the project's JaCoCo at another version the forked JVM gets two `-javaagent` options and dies with `FATAL ERROR in native method: processing of -javaagent failed`. (With the same version JaCoCo replaces its own agent, which is why the coordinate command worked on every fixture.)

Open: how Cupel knows the build runs the agent, and what it runs then.

## Options
**Knowing.** (a) `mvn help:effective-pom`: exact, profiles and all, but a Maven run per detection, possibly downloading the help plugin and parents, at `init`, `doctor`, and `describe` time. (b) A static read of the module's `pom.xml` and its parents, from `relativePath` (default `../pom.xml`) when the POM there is that parent, else from the local repository (`-Dmaven.repo.local=` in `MAVEN_ARGS` or `MAVEN_OPTS`, else `~/.m2/repository`): no process, no network, and the jackson case resolves once Maven has built the project once. (c) Text matches on `pom.xml` (`jacoco-maven-plugin`, `@{argLine}`): wrong for a POM that only manages the plugin or uses it for reports.

**Running.** (a) `mvn test jacoco:report`: the `jacoco` prefix resolves to the build's declared plugin, at the project's version, and its default `report` reads the default `target/jacoco.exec` into `target/site/jacoco/jacoco.xml`, the report ADR-108's producer reads. (b) The coordinate report goal at Cupel's version: reads an exec file written by another version's agent, which JaCoCo does not promise to read.

**Where the choice lives.** A detection rule and a second row in the toolchain table, ahead of `tool.jacoco-maven`, with a producer of its own (`tool.jacoco-maven-project`, the same spec under another ID), so `init`, `describe language java`, and `doctor` show it as they show every other choice; or a branch inside the one producer, which `describe` would not show.

## Decision
1. **A detection rule `{ maven_goal, plugin }`** holds when the root's Maven build runs the plugin's goal itself: some POM of the inheritance chain declares the plugin under `<build><plugins>` (a plugin without a `groupId` is `org.apache.maven.plugins`'), and some POM of the chain gives it an execution naming the goal, there or under `<pluginManagement>`. The chain is read statically as option (b), at most 16 POMs (`crates/cupel-adapt/src/maven_pom.rs`). Profiles, `<inherited>false</inherited>`, a property in a parent's coordinates, a `<localRepository>` set only in `settings.xml`, and a parent neither on disk nor in the local repository are not read; such a build gets the coordinate command, as before.
2. **The Java row's first coverage producer is `tool.jacoco-maven-project`**, detected by `{ maven_goal = "prepare-agent", plugin = "org.jacoco:jacoco-maven-plugin" }`, with `mvn -B --no-transfer-progress test jacoco:report` for `tool.maven`. Every other Maven build keeps `tool.jacoco-maven`'s coordinate command. Both read `target/site/jacoco/jacoco.xml`.
3. **Gradle is unchanged**: `gradle test jacocoTestReport` already runs the build's own `jacoco` plugin, and Cupel adds no agent there.
4. **Fixture**: `crates/cupel-fixtures/repos/jacoco-bound/java/` (a module declaring the plugin bare, a `parent/` managing it at 0.8.15 with `prepare-agent`, Surefire's `@{argLine}`), laid over `repos/coverage/java`'s sources by `crates/cupel-fixtures/tests/jacoco_bound.rs`.

## Consequences
- jackson-core-shaped builds get a coverage command that runs: one agent, the project's, and the project's report goal.
- A `cupel.toml` already holding the coordinate command keeps it; running `init` again writes the project's form.
- A build whose report the project configures elsewhere (`<outputDirectory>`, a non-default `destFile`) leaves no report at `target/site/jacoco/jacoco.xml`, and the run fails with `coverage.run-failed`; the command is the user's to edit.
- `describe language java` lists the second coverage producer and its rule.
