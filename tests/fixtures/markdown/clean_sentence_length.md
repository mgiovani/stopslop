# Deployment Steps

Run the build script before packaging the release.

The pipeline runs three stages.

- Compile the sources.
- Package the artifacts.
- Publish the release.

| Stage | Duration |
| --- | --- |
| Compile | 2 minutes |
| Package | 1 minute |

Each stage logs its own duration to the build report.

The release runbook nests each stage under the step that depends on it.

- The deployment pipeline pulls the latest commit from the release branch and compiles every package in the workspace before it packages artifacts for the staging environment
  - The test runner discovers every spec file under the source tree and executes them in parallel across four worker processes to keep the suite fast
    - The packaging step bundles the compiled binaries together with their configuration files and writes a checksum manifest for every artifact it produces
      - The publish step uploads each artifact to the internal registry and records the build number alongside the commit hash for later lookup
