// IF YOU CHANGE THIS FILE IT MUST BE CHANGED ON BOTH rust-www and rustup.rs

var platforms = ["default", "unknown", "win32", "win64", "win-arm64", "unix"];
var platform_override = null;
var rustup_install_command = "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh";

async function detect_platform() {
    "use strict";

    if (platform_override !== null) {
        return platforms[platform_override];
    }

    var os = "unknown";

    if (navigator.platform == "Linux x86_64") {os = "unix";}
    if (navigator.platform == "Linux i686") {os = "unix";}
    if (navigator.platform == "Linux i686 on x86_64") {os = "unix";}
    if (navigator.platform == "Linux aarch64") {os = "unix";}
    if (navigator.platform == "Linux armv6l") {os = "unix";}
    if (navigator.platform == "Linux armv7l") {os = "unix";}
    if (navigator.platform == "Linux armv8l") {os = "unix";}
    if (navigator.platform == "Linux ppc64") {os = "unix";}
    if (navigator.platform == "Linux mips") {os = "unix";}
    if (navigator.platform == "Linux mips64") {os = "unix";}
    if (navigator.platform == "Linux riscv64") {os = "unix";}
    if (navigator.platform == "Mac") {os = "unix";}
    if (navigator.platform == "Win32") {os = "win32";}
    if (navigator.platform == "Win64" ||
        navigator.userAgent.indexOf("WOW64") != -1 ||
        navigator.userAgent.indexOf("Win64") != -1) { os = "win64"; }
    if (navigator.userAgentData &&
        navigator.userAgentData.platform == "Windows" &&
        await navigator.userAgentData.getHighEntropyValues(["architecture", "bitness"])
            .then(ua => ua.architecture == "arm" && ua.bitness == "64")) { os = "win-arm64"; }
    if (navigator.platform == "FreeBSD x86_64") {os = "unix";}
    if (navigator.platform == "FreeBSD amd64") {os = "unix";}
    if (navigator.platform == "NetBSD x86_64") {os = "unix";}
    if (navigator.platform == "NetBSD amd64") {os = "unix";}
    if (navigator.platform == "SunOS i86pc") {os = "unix";}

    // I wish I knew by now, but I don't. Try harder.
    if (os == "unknown") {
        if (navigator.appVersion.indexOf("Win")!=-1) {os = "win32";}
        if (navigator.appVersion.indexOf("Mac")!=-1) {os = "unix";}
        // rust-www/#692 - FreeBSD epiphany!
        if (navigator.appVersion.indexOf("FreeBSD")!=-1) {os = "unix";}
    }

    // Firefox Quantum likes to hide platform and appVersion but oscpu works
    if (navigator.oscpu) {
        if (navigator.oscpu.indexOf("Win32")!=-1) {os = "win32";}
        if (navigator.oscpu.indexOf("Win64")!=-1) {os = "win64";}
        if (navigator.oscpu.indexOf("Mac")!=-1) {os = "unix";}
        if (navigator.oscpu.indexOf("Linux")!=-1) {os = "unix";}
        if (navigator.oscpu.indexOf("FreeBSD")!=-1) {os = "unix";}
        if (navigator.oscpu.indexOf("NetBSD")!=-1) {os = "unix";}
        if (navigator.oscpu.indexOf("SunOS")!=-1) {os = "unix";}
    }

    return os;
}

function vis(elem, value) {
    var possible = ["block", "inline", "none"];
    for (var i = 0; i < possible.length; i++) {
        if (possible[i] === value) {
            elem.classList.add("display-" + possible[i]);
        } else {
            elem.classList.remove("display-" + possible[i]);
        }
    }
}

async function adjust_for_platform() {
    "use strict";

    var platform = await detect_platform();

    platforms.forEach(function (platform_elem) {
        var platform_div = document.getElementById("platform-instructions-" + platform_elem);
        vis(platform_div, "none");
        if (platform == platform_elem) {
            vis(platform_div, "block");
        }
    });

    adjust_platform_specific_instrs(platform);
}

// NB: This has no effect on rustup.rs
function adjust_platform_specific_instrs(platform) {
    var platform_specific = document.getElementsByClassName("platform-specific");
    for (var el of platform_specific) {
        var el_is_not_win = el.className.indexOf("not-win") !== -1;
        var el_is_inline = el.tagName.toLowerCase() == "span";
        var el_visible_style = "block";
        if (el_is_inline) {
            el_visible_style = "inline";
        }
        if (platform == "win64" || platform == "win32" || platform == "win-arm64") {
            if (el_is_not_win) {
                vis(el, "none");
            } else {
                vis(el, el_visible_style);
            }
        } else {
            if (el_is_not_win) {
                vis(el, el_visible_style);
            } else {
                vis(el, "none");
            }
        }
    }
}

async function cycle_platform() {
    if (platform_override == null) {
        platform_override = 0;
    } else {
        platform_override = (platform_override + 1) % platforms.length;
    }
    await adjust_for_platform();
}

function set_up_cycle_button() {
    var cycle_button = document.getElementById("platform-button");
    cycle_button.onclick = cycle_platform;

    var key="test";
    var idx=0;
    var unlocked=false;

    document.onkeypress = async function(event) {
        if (event.key == "n" && unlocked) {
            await cycle_platform();
        }

        if (event.key == key[idx]) {
            idx += 1;

            if (idx == key.length) {
                vis(cycle_button, "block");
                unlocked = true;
                await cycle_platform();
            }
        } else if (event.key == key[0]) {
            idx = 1;
        } else {
            idx = 0;
        }
    };
}

async function go_to_default_platform() {
    platform_override = 0;
    await adjust_for_platform();
}

// NB: This has no effect on rust-lang.org/install.html
function set_up_default_platform_buttons() {
    var defaults_buttons = document.getElementsByClassName('default-platform-button');
    for (var i = 0; i < defaults_buttons.length; i++) {
        defaults_buttons[i].onclick = go_to_default_platform;
    }
}

function fill_in_bug_report_values() {
    var nav_plat = document.getElementById("nav-plat");
    var nav_app = document.getElementById("nav-app");
    nav_plat.textContent = navigator.platform;
    nav_app.textContent = navigator.appVersion;
}

function process_copy_button_click(id) {
    try {
        navigator.clipboard.writeText(rustup_install_command).then(() =>
          document.getElementById(id).style.opacity = '1');

        setTimeout(() => document.getElementById(id).style.opacity = '0', 3000);
    } catch (e) {
        console.log('Hit a snag when copying to clipboard: ', e);
    }
}

function handle_copy_button_click(e) {
    switch (e.id) {
        case 'copy-button-unix':
            process_copy_button_click('copy-status-message-unix');
            break;
        case 'copy-button-win32':
            process_copy_button_click('copy-status-message-win32');
            break;
        case 'copy-button-win64':
            process_copy_button_click('copy-status-message-win64');
            break;
        case 'copy-button-win-arm64':
            process_copy_button_click('copy-status-message-win-arm64');
            break;
        case 'copy-button-unknown':
            process_copy_button_click('copy-status-message-unknown');
            break;
        case 'copy-button-default':
            process_copy_button_click('copy-status-message-default');
            break;
    }
}

function set_up_copy_button_clicks() {
    var buttons = document.querySelectorAll(".copy-button");
    buttons.forEach(function (element) {
        element.addEventListener('click', function() {
            handle_copy_button_click(element);
        });
    })
}

(async function () {
    await adjust_for_platform();
    set_up_cycle_button();
    set_up_default_platform_buttons();
    set_up_copy_button_clicks();
    fill_in_bug_report_values();
}());
