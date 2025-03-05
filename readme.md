# SwiftAssist

A small CLI tool to fix various XCode build issues and manage templates. Integrates with xcodebuild-server to keep buildServer.json updated when commands are run.

## Commands:
### Basic/Core
```zsh
--quick-clean, -qc
```
cleans build intermediates that can cause problems (PrecompiledHeaders in derived data)

```zsh
--clean, -c
```
cleans derived data and packages and rebuilds project

```zsh
--full-clean, -fc
```
rebuilds project, force purging everything it can (slow)

```zsh
--rebuild, -rb
```
rebuilds the project via xcodebuild on your configured workspace and scheme (set via the config file, if you do not have one this command will create one), then rebuilds the build server

```zsh
--build-server, -bs
```
reconstructs buildServer.json via your configured workspace and scheme

```zsh
--reset-packages, -p
g```
reinstalls spm packages in non-build subdirectories

### Configuration
```zsh
--config, -i
```
sets up a config file

```zsh
--run-deps-script, -d
```
runs a custom script configurable via the config.toml (run -i, edit ~/.config/sass/config.toml)

```zsh
--completions
```
prints zsh completions, add via e.g. "znap fpath _sass 'sass --completions'" for znap

```zsh
--update-templates, -ut
```
copies the contents of ~/.config/sass/templates/ to your xcode templates dir under a 'sass' subfolder, overwriting previous contents


### Fine-grained control
```zsh
--clean-pods, -cp
```
uses swiftcli tools to clean your pods

```zsh
--clean-packages, -cP
```
uses swiftcli tools to clean your packages

```zsh
--wipe-derived, -pd
```
purges derived data

```zsh
--install-packages, -rp
```
uses swiftcli tools to install SPM packages

```zsh
--install-pods, -ip
```
runs pod install (via bundler if detected)

## Completions

Completions can be installed in zsh in the same manner as rust's completions.
For example, if you use znap, add the following to your .zshrc or a sourced script:

```zsh
znap fpath _sass 'sass --completions'
```
