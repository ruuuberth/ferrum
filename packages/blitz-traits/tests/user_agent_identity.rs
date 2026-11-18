//! The `User-Agent` header and the JavaScript `navigator` identity must agree.
//!
//! These are table checks over [`TargetOs`], so the Android row is covered even
//! though this test binary runs on the host. That is the point: an Android
//! build used to send `X11; Linux x86_64` while `navigator.platform` said
//! `Linux armv8l`, a mismatch no page needs to fingerprint for.

use blitz_traits::net::{TARGET_OS, TargetOs, USER_AGENT};

const FIREFOX_PLATFORMS: [TargetOs; 4] = [
    TargetOs::Android,
    TargetOs::Linux,
    TargetOs::MacOs,
    TargetOs::Windows,
];

fn firefox_version(user_agent: &str) -> &str {
    user_agent.split("Firefox/").nth(1).expect("Firefox token")
}

fn rv_version(user_agent: &str) -> &str {
    user_agent
        .split("rv:")
        .nth(1)
        .and_then(|rest| rest.split(')').next())
        .expect("rv token")
}

#[test]
fn android_identity_is_mobile_not_desktop_x86() {
    let user_agent = TargetOs::Android.user_agent();
    assert!(user_agent.contains("Android"), "{user_agent}");
    assert!(user_agent.contains("Mobile"), "{user_agent}");
    assert!(!user_agent.contains("X11"), "{user_agent}");
    assert!(!user_agent.contains("x86_64"), "{user_agent}");
    assert_eq!(TargetOs::Android.platform(), "Linux armv8l");
    assert_eq!(TargetOs::Android.oscpu(), Some("Linux armv8l"));
}

#[test]
fn rv_and_firefox_versions_agree() {
    for os in FIREFOX_PLATFORMS {
        let user_agent = os.user_agent();
        assert_eq!(
            rv_version(user_agent),
            firefox_version(user_agent),
            "rv: and Firefox/ disagree in {user_agent:?}"
        );
    }
}

#[test]
fn platform_tokens_match_the_user_agent() {
    for os in FIREFOX_PLATFORMS {
        let user_agent = os.user_agent();
        let expected = match os {
            TargetOs::Android => "Android",
            TargetOs::Linux => "Linux x86_64",
            TargetOs::MacOs => "Macintosh",
            TargetOs::Windows => "Windows NT 10.0",
            TargetOs::Ios => "iPhone",
        };
        assert!(
            user_agent.contains(expected),
            "{os:?} user agent is missing {expected:?}: {user_agent:?}"
        );
    }
}

#[test]
fn current_target_matches_compile_target() {
    #[cfg(target_os = "android")]
    assert_eq!(TARGET_OS, TargetOs::Android);
    #[cfg(target_os = "linux")]
    assert_eq!(TARGET_OS, TargetOs::Linux);
    #[cfg(target_os = "macos")]
    assert_eq!(TARGET_OS, TargetOs::MacOs);
    #[cfg(target_os = "windows")]
    assert_eq!(TARGET_OS, TargetOs::Windows);
    #[cfg(target_os = "ios")]
    assert_eq!(TARGET_OS, TargetOs::Ios);
    assert_eq!(USER_AGENT, TARGET_OS.user_agent());
}
