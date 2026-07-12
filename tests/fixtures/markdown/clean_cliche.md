# stopslop

stopslop is a command-line linter that checks pull requests for AI-generated
code smells before they reach review. It gives teams the power to catch
sloppy commits early and ship with confidence.

Read the full write-up on our blog: https://example.com/blog/your-journey-to-clean-code

## Installation

Run `cargo install stopslop` to get started. The tool works standalone or
as a pre-commit hook, and it exits non-zero whenever a Tier A rule fires.

## Configuration

Enable extra checks with the select flag, or disable a single rule by code
if it does not fit your project.
