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
