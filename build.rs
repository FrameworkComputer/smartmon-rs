//! Build libsmartmon from the smartmontools submodule with the `cc` crate.
//!
//! Upstream generates config.h, smartmon/smartmon_config.h and
//! smartmon/version.h with autotools. We write them ourselves into OUT_DIR,
//! so that no autotools are needed and MSVC works the same way as GCC/Clang.
//!
//! When updating the submodule, check lib/Makefile.am (source list) and the
//! os_deps/os_libs section in configure.ac for changes.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const SRC: &str = "smartmontools";

/// libsmartmon_la_SOURCES in lib/Makefile.am
const LIB_SOURCES: &[&str] = &[
    "atacmdnames.cpp",
    "atacmds.cpp",
    "dev_ata_cmd_set.cpp",
    "dev_intelliprop.cpp",
    "dev_interface.cpp",
    "dev_jmb39x_raid.cpp",
    "dev_parse_debug.cpp",
    "farmcmds.cpp",
    "hexdump.cpp",
    "knowndrives.cpp",
    "nvmecmds.cpp",
    "json.cpp",
    "scsicmds.cpp",
    "scsiata.cpp",
    "scsinvme.cpp",
    "utility.cpp",
];

struct Target {
    os: String,
    env: String,
    arch: String,
}

impl Target {
    fn windows(&self) -> bool {
        self.os == "windows"
    }
    fn msvc(&self) -> bool {
        self.env == "msvc"
    }
}

fn main() {
    let target = Target {
        os: env::var("CARGO_CFG_TARGET_OS").unwrap(),
        env: env::var("CARGO_CFG_TARGET_ENV").unwrap(),
        arch: env::var("CARGO_CFG_TARGET_ARCH").unwrap(),
    };
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let src = Path::new(SRC);

    if !src.join("lib").exists() {
        panic!("smartmontools submodule missing. Run: git submodule update --init");
    }
    println!("cargo:rerun-if-changed=src/wrapper.cpp");
    println!("cargo:rerun-if-changed={SRC}/lib");
    println!("cargo:rerun-if-changed={SRC}/include");

    let (os_sources, os_libs) = os_deps(&target);

    let config = config_defines(&target);
    let gen_inc = out.join("include");
    fs::create_dir_all(gen_inc.join("smartmon")).unwrap();
    write_config_h(&gen_inc.join("config.h"), &config);
    write_smartmon_config_h(&gen_inc.join("smartmon/smartmon_config.h"), &config);
    write_version_h(&gen_inc.join("smartmon/version.h"), src);

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .include(&gen_inc)
        .include(src.join("include"))
        // knowndrives.cpp includes "drivedb.h"
        .include(src.join("drivedb"))
        .define("HAVE_CONFIG_H", None)
        .define("BUILD_INFO", "\"(smartmon-sys)\"")
        // Only used for the optional user drivedb file /etc/smart_drivedb.h
        .define("SMARTMONTOOLS_SYSCONFDIR", "\"/etc\"")
        .warnings(false);
    if target.msvc() {
        build.flag("/std:c++14").flag("/EHsc");
    } else {
        build.flag("-std=c++11");
    }
    if target.windows() {
        build
            .include(src.join("include/smartmon/os_win32"))
            .include(src.join("include/smartmon/regex"))
            .define("_REGEX_STANDALONE", None)
            .define("_CRT_NONSTDC_NO_DEPRECATE", None)
            .define("_CRT_SECURE_NO_DEPRECATE", None);
    }
    for f in LIB_SOURCES.iter().chain(os_sources.iter()) {
        build.file(src.join("lib").join(f));
    }
    build.file("src/wrapper.cpp");
    build.compile("smartmon");

    // Windows has no regcomp(), use the bundled GNU regex (C code)
    if target.windows() {
        let mut regex = cc::Build::new();
        regex
            .include(&gen_inc)
            .include(src.join("include/smartmon/regex"))
            .define("HAVE_CONFIG_H", None)
            .define("_REGEX_STANDALONE", None)
            .warnings(false)
            .file(src.join("lib/regex/regex.c"));
        if target.msvc() {
            regex.define("_CRT_SECURE_NO_DEPRECATE", None);
        }
        regex.compile("smartmon_regex");
    }

    for lib in os_libs {
        println!("cargo:rustc-link-lib={lib}");
    }
}

/// os_deps and os_libs in configure.ac
fn os_deps(target: &Target) -> (Vec<&'static str>, Vec<&'static str>) {
    match target.os.as_str() {
        "linux" | "android" => (vec!["os_linux.cpp", "cciss.cpp", "dev_areca.cpp"], vec![]),
        "freebsd" => (
            vec!["os_freebsd.cpp", "cciss.cpp", "dev_areca.cpp"],
            vec!["cam", "sbuf", "usb"],
        ),
        "netbsd" => (vec!["os_netbsd.cpp"], vec!["util"]),
        "openbsd" => (vec!["os_openbsd.cpp"], vec!["util"]),
        "macos" => (
            vec!["os_darwin.cpp"],
            vec!["framework=CoreFoundation", "framework=IOKit"],
        ),
        "windows" => (
            vec![
                "os_win32.cpp",
                "dev_areca.cpp",
                "os_win32/wmiquery.cpp",
                "os_win32/popen_win32.cpp",
            ],
            vec!["ole32", "oleaut32", "advapi32", "user32"],
        ),
        _ => (vec!["os_generic.cpp"], vec![]),
    }
}

/// Subset of what configure would detect, only what lib/ actually uses
fn config_defines(target: &Target) -> Vec<(&'static str, String)> {
    let q = |s: &str| format!("\"{s}\"");
    let version = package_version();
    let mut d = vec![
        ("PACKAGE", q("smartmontools")),
        ("PACKAGE_NAME", q("smartmontools")),
        ("PACKAGE_VERSION", q(&version)),
        ("PACKAGE_STRING", q(&format!("smartmontools {version}"))),
        (
            "PACKAGE_BUGREPORT",
            q("smartmontools-support@listi.jpberlin.de"),
        ),
        ("PACKAGE_TARNAME", q("smartmontools")),
        ("PACKAGE_URL", q("https://www.smartmontools.org/")),
        ("VERSION", q(&version)),
        ("SMARTMONTOOLS_BUILD_HOST", q(&env::var("TARGET").unwrap())),
        ("SMARTMONTOOLS_CONFIGURE_ARGS", q(" [smartmon-sys]")),
        ("HAVE_LOCALE_H", "1".into()),
    ];
    if target.os == "freebsd" {
        d.push(("CISS_LOCATION", q("cissio_freebsd.h")));
    }
    if !target.msvc() {
        d.push(("HAVE_ATTR_PACKED", "1".into()));
    }
    if !target.windows() {
        d.push(("HAVE_UNISTD_H", "1".into()));
    }
    if target.os == "linux" || target.os == "android" {
        d.push(("HAVE_BYTESWAP_H", "1".into()));
        d.push(("HAVE_SYS_SYSMACROS_H", "1".into()));
    }
    let is_64bit = env::var("CARGO_CFG_TARGET_POINTER_WIDTH").unwrap() == "64";
    if !target.msvc() && is_64bit {
        d.push(("HAVE___INT128", "1".into()));
    }
    if !target.msvc() && (target.arch == "x86_64" || target.arch == "x86") {
        d.push(("HAVE_LONG_DOUBLE_WIDER", "1".into()));
    }
    d
}

fn write_config_h(path: &Path, defines: &[(&str, String)]) {
    let mut s = String::from("/* Generated by smartmon-sys build.rs */\n");
    for (k, v) in defines {
        s += &format!("#define {k} {v}\n");
    }
    fs::write(path, s).unwrap();
}

/// Same as config.h but with SMARTMON_ prefix, like include/Makefile.am does it
fn write_smartmon_config_h(path: &Path, defines: &[(&str, String)]) {
    let mut s = String::from(
        "/* Generated by smartmon-sys build.rs */\n\
         #ifndef _SMARTMON_CONFIG_H\n#define _SMARTMON_CONFIG_H\n",
    );
    for (k, v) in defines {
        let k = if k.starts_with("SMARTMON") {
            k.to_string()
        } else {
            format!("SMARTMON_{k}")
        };
        s += &format!("#define {k} {v}\n");
    }
    s += "#endif // _SMARTMON_CONFIG_H\n";
    fs::write(path, s).unwrap();
}

/// Replaces util/getversion.sh
fn write_version_h(path: &Path, src: &Path) {
    let version = package_version();
    let mut s = String::from("/* Generated by smartmon-sys build.rs */\n");
    s += &format!("#define SMARTMONTOOLS_PKG_VER \"{version}\"\n");
    match git_rev(src) {
        Some((rev, date, time)) => {
            s += &format!("#define SMARTMONTOOLS_GIT_REV \"{rev}\"\n");
            s += &format!("#define SMARTMONTOOLS_GIT_REV_DATE \"{date}\"\n");
            s += &format!("#define SMARTMONTOOLS_GIT_REV_TIME \"{time}\"\n");
            s += &format!("#define SMARTMONTOOLS_GIT_VER_DESC \"{version}-g{rev}\"\n");
        }
        None => s += &format!("#define SMARTMONTOOLS_GIT_VER_DESC \"{version}\"\n"),
    }
    fs::write(path, s).unwrap();
}

/// Version from AC_INIT([smartmontools],[x.y],...) in configure.ac
fn package_version() -> String {
    let ac = fs::read_to_string(Path::new(SRC).join("configure.ac")).unwrap();
    ac.lines()
        .find_map(|l| l.strip_prefix("AC_INIT([smartmontools],["))
        .and_then(|l| l.split(']').next())
        .expect("AC_INIT not found in configure.ac")
        .to_string()
}

/// (short rev, UTC date, UTC time) of the submodule checkout, if it's a git checkout
fn git_rev(src: &Path) -> Option<(String, String, String)> {
    let out = Command::new("git")
        .arg("-C")
        .arg(src)
        .args([
            "log",
            "-1",
            "--date=format-local:%Y-%m-%d %H:%M:%S",
            "--format=%h %cd",
            "--abbrev=12",
        ])
        .env("TZ", "UTC")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let mut it = s.split_whitespace();
    Some((
        it.next()?.to_string(),
        it.next()?.to_string(),
        it.next()?.to_string(),
    ))
}
