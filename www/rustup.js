var platform_override = null;

function detect_platform() {
    "use strict";

    if (platform_override) {
        return platform_override;
    }

    var os = "unknown";

    if (navigator.platform == "Linux x86_64") {os = "unix";}
    if (navigator.platform == "Linux i686") {os = "unix";}
    if (navigator.platform == "Linux i686 on x86_64") {os = "unix";}
    if (navigator.platform == "Linux aarch64") {os = "unix";}
    if (navigator.platform == "Linux armv6l") {os = "unix";}
    if (navigator.platform == "Linux armv7l") {os = "unix";}
    if (navigator.platform == "Linux ppc64") {os = "unix";}
    if (navigator.platform == "Mac") {os = "unix";}
    if (navigator.platform == "Win32") {os = "win";}
    if (navigator.platform == "FreeBSD x86_64") {os = "unix";}
    if (navigator.platform == "FreeBSD amd64") {os = "unix";}
    if (navigator.platform == "NetBSD x86_64") {os = "unix";}
    if (navigator.platform == "NetBSD amd64") {os = "unix";}

    if (navigator.platform == "Linux armv7l"
        && navigator.appVersion.indexOf("Android") != -1 ) {
        os = "android";
    }

    // I wish I knew by now, but I don't. Try harder.
    if (os == "unknown") {
        if (navigator.appVersion.indexOf("Win")!=-1) {os = "win";}
        if (navigator.appVersion.indexOf("Mac")!=-1) {os = "unix";}
    }

    return os;
}

function adjust_for_platform() {
    "use strict";

    var platform = detect_platform();

    var unix_div = document.getElementById("platform-instructions-unix");
    var win_div = document.getElementById("platform-instructions-win");
    var android_div = document.getElementById("platform-instructions-android");
    var unknown_div = document.getElementById("platform-instructions-unknown");
    var default_div = document.getElementById("platform-instructions-default");

    unix_div.style.display = "none";
    win_div.style.display = "none";
    android_div.style.display = "none";
    unknown_div.style.display = "none";
    default_div.style.display = "none";

    if (platform == "unix") {
        unix_div.style.display = "block";
    } else if (platform == "win") {
        win_div.style.display = "block";
    } else if (platform == "android") {
        android_div.style.display = "block";
    } else if (platform == "unknown") {
        unknown_div.style.display = "block";
    } else {
        default_div.style.display = "block";
    }
}

function cycle_platform() {
    if (platform_override == null) {
        platform_override = "default";
    } else if (platform_override == "default") {
        platform_override = "unknown";
    } else if (platform_override == "unknown") {
        platform_override = "win";
    } else if (platform_override == "win") {
        platform_override = "unix";
    } else if (platform_override == "unix") {
        platform_override = "android";
    } else if (platform_override == "android") {
        platform_override = "default";
    }
    adjust_for_platform();
}

function set_up_cycle_button() {
    var cycle_button = document.getElementById("platform-button");
    cycle_button.onclick = cycle_platform;

    var key="test";
    var idx=0;
    var unlocked=false;

    document.onkeypress = function(event) {
        if (event.key == "n" && unlocked) {
            cycle_platform();
        }

        if (event.key == key[idx]) {
            idx += 1;

            if (idx == key.length) {
                cycle_button.style.display = "block";
                unlocked = true;
            }
        } else if (event.key == key[0]) {
            idx = 1;
        } else {
            idx = 0;
        }
    };
}

function fill_in_bug_report_values() {
    var nav_plat = document.getElementById("nav-plat");
    var nav_app = document.getElementById("nav-app");
    nav_plat.textContent = navigator.platform;
    nav_app.textContent = navigator.appVersion;
}

(function () {
    adjust_for_platform();
    set_up_cycle_button();
    fill_in_bug_report_values();
}());
