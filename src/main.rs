use std::fs;
use std::fs::OpenOptions;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::env;
use std::process::Command;
use std::{thread, time};

use copy_dir::copy_dir;
use regex::Regex;
use toml::Table;
use file_lock::{FileOptions, FileLock};
use walkdir::WalkDir;

#[derive(Clone)]
struct Config {
    post_install_script_location: Option<String>,
    scheme: Option<String>,
    workspace_name: Option<String>,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        help();
    }
    for item in args.iter().skip(1) {
        exec_arg(item)
    }
}

fn exec_arg(arg: &str) {
    match arg {
        "-h" => help(),
        "--help" => help(),
        "-qc" => quick_clean(),
        "--quick-clean" => quick_clean(),
        "-c" => clean(),
        "--clean" => clean(),
        "-fc" => full_clean(),
        "--full-clean" => full_clean(),
        "-rb" => rebuild(),
        "--rebuild" => rebuild(),
        "-bs" => rebuild_build_server(),
        "--build-server" => rebuild_build_server(),
        "-i" => setup_config_file(),
        "--config" => setup_config_file(),
        "-cp" => wipe_pods(),
        "--clean-pods" => wipe_pods(),
        "-cP" => clean_packages(),
        "--clean-packages" => clean_packages(),
        "-ph" => wipe_pod_cache_hard(),
        "--wipe-pods" => wipe_pod_cache_hard(),
        "-pd" => wipe_derived_data(false),
        "--wipe-derived" => wipe_derived_data(false),
        "-rp" => install_packages(),
        "--install-packages" => install_packages(),
        "-ip" => install_pods(),
        "--install-pods" => install_pods(),
        "-d" => install_deps_script().expect("Error"),
        "--run-deps-script" => install_deps_script().expect("Error"),
        "-p" => reset_packages(),
        "--reset-packages" => reset_packages(),
        "-ut" => update_templates(),
        "--update-templates" => update_templates(),
        "--completions" => echo_completions(),
        "-t" => test(),
        _ => help(),
    }
}

fn help() {
    let help_message = include_str!("help.txt");
    println!("{help_message}");
    std::process::exit(0);
}

fn echo_completions() {
    let completions = include_str!("_sass");
    println!("{completions}");
    std::process::exit(0);
}

fn reset_packages() {
    clean_packages();
    install_packages();
}

fn quick_clean() {
    wipe_derived_data(true);
}

fn clean() {
    clean_packages();
    wipe_derived_data(false);
    install_packages();
    rebuild();
    rebuild_build_server();
}

fn full_clean() {
    clean_packages();
    wipe_pod_cache_hard();
    wipe_derived_data(false);
    install_deps_script();
    install_packages();
    let dur = time::Duration::from_millis(999);
    thread::sleep(dur);
    install_pods();
    rebuild();
    rebuild_build_server();
}

fn test() {
    let ret = _uses_bundler();
    println!("{}", ret);
}

fn rebuild() {
    println!("Building...");
    let config = setup_and_get_config();
    let gitroot = git_root();
    let pods_dir = gitroot.clone() + "/.bundle/";
    match fs::remove_dir_all(pods_dir) {
        Ok(_result) => (),
        Err(error) => println!("Error: {}", error),
    }
    let workspace = config.workspace_name.expect("No workspace name found!");
    let scheme = config.scheme.expect("No scheme found!");
    let output = Command::new("xcodebuild")
        .args(["-workspace", format!("{}.xcworkspace", workspace).as_str(), "-scheme", scheme.as_str(), "-destination", r"generic/platform=iOS Simulator", "-resultBundlePath", ".bundle", "OTHER_CFLAGS=\"-DCMAKE_C_COMPILER_LAUNCHER=$(which sccache) -DCMAKE_CXX_COMPILER_LAUNCHER=$(which sccache)\""])
        .current_dir(gitroot)
        .output()
        .expect("failed to execute process");
    println!("{}", String::from_utf8(output.stdout).expect("Error executing build"));
    rebuild_build_server();
}

fn rebuild_build_server() {
    println!("Generating buildServer.json...");
    let config = setup_and_get_config();
    let gitroot = git_root();
    let workspace = config.workspace_name.expect("No workspace name found!");
    let scheme = config.scheme.expect("No scheme found!");
    let output = Command::new("xcode-build-server")
        .args(["config", "-workspace", format!("{}.xcworkspace", workspace).as_str(), "-scheme", scheme.as_str()])
        .current_dir(gitroot)
        .output()
        .expect("failed to execute process");
    println!("{}", String::from_utf8(output.stdout).expect("Error constructing build server"));
}

fn install_deps_script() -> Option<()> {
    println!("Executing dependency installation script.");
    let config = setup_and_get_config();
    let git_root = git_root();
    let script_dir = git_root.clone() + config.post_install_script_location?.as_str();
    Command::new("sh")
        .args([script_dir])
        .current_dir(git_root)
        .output()
        .expect("failed to execute process");
    Some(())
}

fn wipe_derived_data(intermediates_only: bool) {
    println!("Cleaning DerivedData...");
    let paths = get_derived_data_folders().unwrap_or_else(|_| Vec::new());
    let xcode_dd_search = Regex::new(r"^.*-.*$").expect("DerivedData regex failed to parse");
    for path in paths {
        let reg_str = path.to_str().expect("Unusual string format in derived data directory!");
        let correct_format = match xcode_dd_search.captures(reg_str) {
            Some(_expr) => true,
            None => false,
        };
        if !correct_format {
            continue
        }

        let mut mut_path = path.clone();
        mut_path.push("info.plist");
        let lock_for_writing = FileOptions::new().write(true).create_new(false);
        let lock = match FileLock::lock(mut_path, true, lock_for_writing) {
            Ok(lock) => lock,
            Err(_err) => panic!("Error locking derived data!"),
        };
        let _= lock.unlock();
        let retry_dur = time::Duration::from_millis(999);
        let retry_cap = 15;
        let mut target_path = path;
        if intermediates_only {
            target_path.push("Build");
            target_path.push("Intermediates.noindex");
            target_path.push("PrecompiledHeaders");
        }
        for i in 1..retry_cap {
            match fs::remove_dir_all(target_path.clone()) {
                Ok(_some) => return,
                Err(error) => println!("Error: {}. Directory could be locked, retrying. Attempt {} of 15.", error, i),
            }
            thread::sleep(retry_dur);
        }
        println!("Failed to lock directory.");
    }
}

fn get_derived_data_folders() -> io::Result<Vec<PathBuf>> {
    let derived_data_str = shellexpand::tilde("~/Library/Developer/Xcode/DerivedData/").into_owned().to_string();
    let entries = fs::read_dir(derived_data_str)?
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>, io::Error>>()?;

    Ok(entries)
}

fn wipe_pod_cache_hard() {
    println!("Hard clearing pod cache...");
    let gitroot = git_root();
    let pods_dir = gitroot.clone() + "/Pods/";
    let lockfile_path = gitroot + "/Podfile.lock";
    let cocoa_dir_string = shellexpand::tilde("~/Library/Caches/CocoaPods/").into_owned().to_string();
    let lock_for_writing = FileOptions::new().write(true).create_new(false);

    let lock = match FileLock::lock(lockfile_path.clone(), true, lock_for_writing) {
        Ok(lock) => lock,
        Err(_err) => panic!("Error locking derived data!"),
    };
    _ = lock.unlock();
    match fs::remove_file(lockfile_path) {
        Ok(_result) => (),
        Err(error) => println!("Error: {}", error),
    }
    match fs::remove_dir_all(cocoa_dir_string) {
        Ok(_result) => (),
        Err(error) => println!("Error: {}", error),
    }
    match fs::remove_dir_all(pods_dir) {
        Ok(_result) => (),
        Err(error) => println!("Error: {}", error),
    }
}

fn wipe_pods() {
    println!("Clearing pod cache...");
    let uses_bundler = _uses_bundler();
    if uses_bundler {
        Command::new("bundle")
            .args(["exec", "pod", "cache", "clean", "--all"])
            .current_dir(git_root())
            .output()
            .expect("failed to execute process");
    } else {
        Command::new("pod")
            .args(["cache", "clean", "--all"])
            .current_dir(git_root())
            .output()
            .expect("failed to execute process");
    }
}

fn install_pods() {
    println!("Installing pods...");
    let uses_bundler = _uses_bundler();
    if uses_bundler {
        let output = Command::new("bundle")
            .args(["exec", "pod", "install", "--repo-update"])
            .current_dir(git_root())
            .output()
            .expect("failed to execute process");
        let output_str = String::from_utf8(output.stdout).expect("Did not decode properly.");
        println!("{}", output_str);
    } else {
        let output = Command::new("pod")
            .args(["install", "--repo-update"])
            .current_dir(git_root())
            .output()
            .expect("failed to execute process");
        let output_str = String::from_utf8(output.stdout).expect("Did not decode properly.");
        println!("{}", output_str);
    }
}

// TODO: apply to subprojects
fn clean_packages() {
    let root = git_root();
    let package_regex = Regex::new(r"ackage\.swift").expect("Package search regex failed to parse");
    let build_regex = Regex::new(r"\.build").expect("Build dir search regex failed to parse");
    let mut package_roots: Vec<PathBuf> = Vec::new();
    let walker = WalkDir::new(root);
    for entry in walker {
        let path = entry.unwrap().into_path();

        let has_package = match package_regex.captures(&path.to_string_lossy()) {
            Some(_expr) => true,
            None => false,
        };
        let has_build_dir = match build_regex.captures(&path.to_string_lossy()) {
            Some(_expr) => true,
            None => false,
        };
        if has_package && !has_build_dir {
            package_roots.push(path);
        }
    }

    for mut package_loc in package_roots {
        package_loc.pop();
        println!("Executing package clean in {}", &package_loc.to_string_lossy());
        Command::new("swift")
            .args(["package", "purge-cache"])
            .current_dir(&package_loc)
            .output()
            .expect("failed to execute process");
        Command::new("swift")
            .args(["package", "reset"])
            .current_dir(&package_loc)
            .output()
            .expect("failed to execute process");
        Command::new("swift")
            .args(["package", "clean"])
            .current_dir(&package_loc)
            .output()
            .expect("failed to execute process");
    }
}

fn install_packages() {
    let root = git_root();
    let package_regex = Regex::new(r"ackage\.swift").expect("Package search regex failed to parse");
    let build_regex = Regex::new(r"\.build").expect("Build dir search regex failed to parse");
    let mut package_roots: Vec<PathBuf> = Vec::new();
    let walker = WalkDir::new(root);
    for entry in walker {
        let path = entry.unwrap().into_path();

        let has_package = match package_regex.captures(&path.to_string_lossy()) {
            Some(_expr) => true,
            None => false,
        };
        let has_build_dir = match build_regex.captures(&path.to_string_lossy()) {
            Some(_expr) => true,
            None => false,
        };
        if has_package && !has_build_dir {
            package_roots.push(path);
        }
    }

    for mut package_loc in package_roots {
        package_loc.pop();
        println!("Executing package build in {}", &package_loc.to_string_lossy());
        Command::new("swift")
            .args(["package", "resolve"])
            .current_dir(&package_loc)
            .output()
            .expect("failed to execute process");
        Command::new("swift")
            .args(["package", "update"])
            .current_dir(&package_loc)
            .output()
            .expect("failed to execute process");
    }
}

// Checks if you use a bundler
fn _uses_bundler() -> bool {
    let output = Command::new("gem")
        .args(["list", "--local"])
        .output()
        .expect("failed to execute process");

    let gems = String::from_utf8(output.stdout).unwrap_or(String::from(""));
    let bundler_search = Regex::new(r"bundler").expect("Bundler regex failed to parse");
    let has_bundler = match bundler_search.captures(&gems) {
        Some(_expr) => true,
        None => false,
    };

    let gemfile_path_string = git_root() + "/Gemfile";
    let path = Path::new(&gemfile_path_string);
    let exists = fs::metadata(path).is_ok();
    exists && has_bundler
}

fn git_root() -> String {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .expect("failed to execute process");
    let mut str = String::from_utf8(output.stdout).unwrap_or(String::from(""));
    str.pop();
    str
}

fn setup_and_get_config() -> Config {
    setup_config_file();
    load_config()
}

fn update_templates() {
    println!("Updating templates.");
    let dir_path_string = shellexpand::tilde("~/.config/sass/templates/").into_owned().to_string();
    let xcode_path_string = shellexpand::tilde("~/Library/Developer/Xcode/Templates").into_owned().to_string();
    let dir_path = Path::new(&dir_path_string);
    let xcode_path = Path::new(&xcode_path_string);
    let dir_exists = fs::metadata(dir_path).is_ok() && fs::metadata(xcode_path).is_ok();
    if dir_exists {
        let mut xcode_pathbuf = xcode_path.to_path_buf();
        xcode_pathbuf.push("sass");
        _ = fs::remove_dir_all(&xcode_pathbuf);
        match copy_dir(dir_path, xcode_pathbuf.as_path()) {
            Ok(_result) => (),
            Err(error) => println!("Error: {}", error),
        }
    }
}

fn setup_config_file() {
    println!("Setting up configuration file.");
    let dir_path_string = shellexpand::tilde("~/.config/sass/").into_owned().to_string();
    let dir_path = Path::new(&dir_path_string);
    let dir_exists = fs::metadata(dir_path).is_ok();
    let config_path_string = shellexpand::tilde("~/.config/sass/config.toml").into_owned().to_string();

    let config_path = Path::new(&config_path_string);
    let config_file_exists = fs::metadata(config_path).is_ok();
    if !dir_exists {
        if let Err(why) = fs::create_dir(dir_path) {
            println!("! {:?}", why.kind());
        }
    }
    if !config_file_exists {
        touch(config_path).unwrap_or_else(|why| {
            println!("! {:?}", why.kind());
        });
        let default_config = "#Relative to project's git root\n\
            post_install_script_location = \"/scripts/install_dependencies.sh\"\n\n\
            workspace_name = \"\"\n\
            scheme = \"\"\n";
        fs::write(config_path, default_config).expect("echo \"Unable to write config file.\"")
    }
}

fn load_config() -> Config {
    let config_path_string = shellexpand::tilde("~/.config/sass/config.toml").into_owned().to_string();
    let config_path = Path::new(&config_path_string);
    let config_contents = fs::read_to_string(config_path).expect("echo \"Could not read config.toml!\"");
    let config = config_contents.parse::<Table>().expect("echo \"Could not parse config.toml!\"");
    let mut post_install_script_location: Option<String> = None;
    if config.contains_key("post_install_script_location") {
        let post_install_script_location_str = config["post_install_script_location"].as_str();
        if let Some(unwrap) = post_install_script_location_str {
            post_install_script_location = Some(unwrap.to_string())
        }
    }
    let mut scheme: Option<String> = None;
    if config.contains_key("scheme") {
        let scheme_str = config["scheme"].as_str();
        if let Some(unwrap) = scheme_str {
            scheme = Some(unwrap.to_string())
        }
    }
    let mut workspace_name: Option<String> = None;
    if config.contains_key("workspace_name") {
        let workspace_name_str = config["workspace_name"].as_str();
        if let Some(unwrap) = workspace_name_str {
            workspace_name = Some(unwrap.to_string())
        }
    }
    Config {
        post_install_script_location,
        scheme,
        workspace_name
    }
}

fn touch(path: &Path) -> io::Result<()> {
    match OpenOptions::new().create(true).truncate(false).write(true).open(path) {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}
