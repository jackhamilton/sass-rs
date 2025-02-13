use std::fs;
use std::fs::OpenOptions;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::env;
use std::process::Command;

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
        panic!("Not enough arguments! Try -h for help.");
    }
    let arg = args[1].clone();
    match arg.as_str() {
        "-h" => help(),
        "-i" => setup_config_file(),
        "-wp" => wipe_pods(),
        "-wP" => clean_packages(),
        "-wh" => wipe_pod_cache_hard(),
        "-wd" => wipe_derived_data(),
        "-rp" => install_packages(),
        "-d" => install_deps_script().expect("Error"),
        "-t" => test(),
        _ => help(),
    }
}

fn help() {
    println!("Arguments:\n\t-h for help (this)\n\teval $(themester -r) to randomize your theme\
        \n\teval $(themester -l) in your .zshrc to load the last session's theme environment variables");
    std::process::exit(0);
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
    None
}

fn wipe_derived_data() {
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
        fs::remove_dir_all(path).expect("Error deleting derived data!");
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
    let lockfile_path = gitroot + "/Pods/";
    let cocoa_dir_string = shellexpand::tilde("~/Library/Caches/CocoaPods/").into_owned().to_string();
    let lock_for_writing = FileOptions::new().write(true).create_new(false);

    println!("{}", lockfile_path);
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
    let bundler_search = Regex::new(r"^bundler.*$").expect("Bundler regex failed to parse");
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
