# ABOUTME: The release's tokens as 1Password secret references (ADR-238), PyPI's alone (npm signs in through the browser), for `op run --env-file=release.env.tpl -- make publish V=X.Y.Z`:
# ABOUTME: op resolves each one into that command's environment alone. It holds references only; a value never goes in this file.
PYPI_TOKEN=op://Cupel/prod-cupel-pypi-api-token/credential
