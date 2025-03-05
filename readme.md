# SwiftAssist

A small CLI tool to fix various XCode build issues and manage templates. Integrates with xcodebuild-server to keep buildServer.json updated when commands are run.

## Commands:
### Basic/Core
--quick-clean, -qc => cleans build intermediates that can cause problems
--clean, -c => cleans derived data and packages and rebuilds project
--full-clean, -fc => rebuilds project, force purging everything it can (slow)
--rebuild, -rb => rebuilds the project via xcodebuild on your configured workspace and scheme, then rebuilds the build server
--build-server, -bs => reconstructs buildServer.json via your configured workspace and scheme
--reset-packages, -p => reinstalls spm packages in non-build subdirectories

### Configuration
--config, -i => sets up a config file
--run-deps-script, -d => runs a custom script configurable via the config.toml (run -i, edit ~/.config/sass/config.toml)
--completions => prints zsh completions, add via e.g. "znap fpath _sass 'sass --completions'" for znap
--update-templates, -ut => copies the contents of ~/.config/sass/templates/ to your xcode templates dir under a 'sass' subfolder, overwriting previous contents

### Fine-grained control
--clean-pods, -cp => uses swiftcli tools to clean your pods
--clean-packages, -cP => uses swiftcli tools to clean your packages
--wipe-derived, -pd => purges derived data
--install-packages, -rp => uses swiftcli tools to install SPM packages
--install-pods, -ip => runs pod install (via bundler if detected)

## Completions

Completions can be installed in zsh in the same manner as rust's completions.
For example, if you use znap, add the following to your .zshrc or a sourced script:

```zsh
znap fpath _sass 'sass --completions'
```
