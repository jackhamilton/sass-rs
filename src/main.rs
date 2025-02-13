use std::fs;
use std::fs::OpenOptions;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::env;
use std::process::Command;
use std::{thread, time};

use regex::Regex;
use toml::Table;
use file_lock::{FileOptions, FileLock};

#[derive(Clone)]
struct Config {
    post_install_script_location: Option<String>
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        help();
    }
    let arg = args[1].clone();
    match arg.as_str() {
        "-h" => help(),
        "-c" => quick_clean(),
        "-fc" => full_clean(),
        "-i" => setup_config_file(),
        "-wp" => wipe_pods(),
        "-wP" => clean_packages(),
        "-wh" => wipe_pod_cache_hard(),
        "-wd" => wipe_derived_data(false),
        "-rp" => install_packages(),
        "-ip" => install_pods(),
        "-d" => install_deps_script().expect("Error"),
        "-t" => test(),
        _ => help(),
    }
}

fn help() {
    println!("\
        -h => print this help menu\n\
        -c => cleans build intermediates that can cause problems\n\
        -fc => cleans everything it can get its hands on (slow)\n\n\
        -i => sets up a config file (allows a custom script to be executed on -fc end before pod install and package install run)\n\
        -cp => uses swiftcli tools to clean your pods\n\
        -cP => uses swiftcli tools to clean your packages\n\
        -pp => manually purges pod artifacts\n\
        -pd => purges derived data\n\
        -rp => uses swiftcli tools to install SPM packages\n\
        -ip => runs pod install (via bundler if detected)\n\
        -d => runs a custom script configurable via the config.toml (run -i, edit ~/.config/sass/config.toml)\n\n\
    ");
    std::process::exit(0);
}

fn quick_clean() {
    wipe_derived_data(true)
}

fn full_clean() {
    wipe_pods();
    clean_packages();
    wipe_pod_cache_hard();
    wipe_derived_data(false);
    install_deps_script();
    install_packages();
    install_pods();
}

fn test() {
    let ret = _uses_bundler();
    println!("{}", ret);
}

fn install_deps_script() -> Option<()> {
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
        let retry_cap = 14;
        let mut target_path = path;
        if intermediates_only {
            target_path.push("Build");
            target_path.push("Intermediates.noindex");
            target_path.push("PrecompiledHeaders");
        }
        for i in -1..retry_cap {
            match fs::remove_dir_all(target_path.clone()) {
                Ok(_some) => return,
                Err(error) => println!("Error: {}. Directory could be locked, retrying. Attempt {} of 14.", error, i),
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
    let uses_bundler = _uses_bundler();
    if uses_bundler {
        Command::new("bundle")
            .args(["exec", "pod", "install", "--repo-update"])
            .current_dir(git_root())
            .output()
            .expect("failed to execute process");
    } else {
        Command::new("pod")
            .args(["install", "--repo-update"])
            .current_dir(git_root())
            .output()
            .expect("failed to execute process");
    }
}

// TODO: apply to subprojects
fn clean_packages() {
    let root = git_root();
    Command::new("swift")
        .args(["package", "purge-cache"])
        .current_dir(root.clone())
        .output()
        .expect("failed to execute process");
    Command::new("swift")
        .args(["package", "reset"])
        .current_dir(root.clone())
        .output()
        .expect("failed to execute process");
    Command::new("swift")
        .args(["package", "clean"])
        .current_dir(root)
        .output()
        .expect("failed to execute process");
}

fn install_packages() {
    let root = git_root();
    Command::new("swift")
        .args(["package", "resolve"])
        .current_dir(root.clone())
        .output()
        .expect("failed to execute process");
    Command::new("swift")
        .args(["package", "update"])
        .current_dir(root.clone())
        .output()
        .expect("failed to execute process");
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

fn setup_config_file() {
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
            post_install_script_location = \"/scripts/install_dependencies.sh\"\n\n";
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
    Config {
        post_install_script_location
    }
}

fn touch(path: &Path) -> io::Result<()> {
    match OpenOptions::new().create(true).truncate(false).write(true).open(path) {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}
