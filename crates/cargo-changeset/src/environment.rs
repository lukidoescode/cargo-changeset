use std::io::IsTerminal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NonInteractiveReason {
    ExplicitDisable,
    CiDetected { env_var: String },
    NoTerminal,
}

pub fn is_interactive() -> bool {
    non_interactive_reason().is_none()
}

pub fn non_interactive_reason() -> Option<NonInteractiveReason> {
    if std::env::var("CARGO_CHANGESET_NO_TTY").is_ok() {
        return Some(NonInteractiveReason::ExplicitDisable);
    }

    if std::env::var("CARGO_CHANGESET_FORCE_TTY").is_ok() {
        return None;
    }

    if let Some(env_var) = detect_ci_env_var() {
        return Some(NonInteractiveReason::CiDetected { env_var });
    }

    if !std::io::stdin().is_terminal() {
        return Some(NonInteractiveReason::NoTerminal);
    }

    None
}

fn detect_ci_env_var() -> Option<String> {
    const CI_ENV_VARS: &[&str] = &[
        "CI",
        "GITHUB_ACTIONS",
        "GITLAB_CI",
        "CIRCLECI",
        "TRAVIS",
        "JENKINS_URL",
        "BUILDKITE",
        "TF_BUILD",
    ];

    for var in CI_ENV_VARS {
        if std::env::var(var).is_ok() {
            return Some((*var).to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn with_env<F, R>(vars: &[(&str, &str)], clear: &[&str], f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let _guard = ENV_MUTEX.lock().expect("mutex poisoned");

        let mut old_values: Vec<(&str, Option<String>)> = Vec::new();

        for var in clear {
            old_values.push((var, std::env::var(var).ok()));
            // SAFETY: Test code runs sequentially with ENV_MUTEX held.
            unsafe { std::env::remove_var(var) };
        }

        for (key, value) in vars {
            old_values.push((key, std::env::var(key).ok()));
            // SAFETY: Test code runs sequentially with ENV_MUTEX held.
            unsafe { std::env::set_var(key, value) };
        }

        let result = f();

        for (key, old_value) in old_values {
            match old_value {
                // SAFETY: Test code runs sequentially with ENV_MUTEX held.
                Some(v) => unsafe { std::env::set_var(key, v) },
                // SAFETY: Test code runs sequentially with ENV_MUTEX held.
                None => unsafe { std::env::remove_var(key) },
            }
        }

        result
    }

    const ALL_CI_VARS: &[&str] = &[
        "CI",
        "GITHUB_ACTIONS",
        "GITLAB_CI",
        "CIRCLECI",
        "TRAVIS",
        "JENKINS_URL",
        "BUILDKITE",
        "TF_BUILD",
        "CARGO_CHANGESET_NO_TTY",
        "CARGO_CHANGESET_FORCE_TTY",
    ];

    mod detect_ci_env_var {
        use super::*;

        #[test]
        fn returns_none_when_no_ci_vars_set() {
            with_env(&[], ALL_CI_VARS, || {
                assert!(detect_ci_env_var().is_none());
            });
        }

        #[test]
        fn detects_ci() {
            with_env(&[("CI", "true")], ALL_CI_VARS, || {
                assert_eq!(detect_ci_env_var(), Some("CI".to_string()));
            });
        }

        #[test]
        fn detects_github_actions() {
            with_env(&[("GITHUB_ACTIONS", "true")], ALL_CI_VARS, || {
                assert_eq!(detect_ci_env_var(), Some("GITHUB_ACTIONS".to_string()));
            });
        }

        #[test]
        fn detects_gitlab_ci() {
            with_env(&[("GITLAB_CI", "true")], ALL_CI_VARS, || {
                assert_eq!(detect_ci_env_var(), Some("GITLAB_CI".to_string()));
            });
        }

        #[test]
        fn detects_circleci() {
            with_env(&[("CIRCLECI", "true")], ALL_CI_VARS, || {
                assert_eq!(detect_ci_env_var(), Some("CIRCLECI".to_string()));
            });
        }

        #[test]
        fn detects_travis() {
            with_env(&[("TRAVIS", "true")], ALL_CI_VARS, || {
                assert_eq!(detect_ci_env_var(), Some("TRAVIS".to_string()));
            });
        }

        #[test]
        fn detects_jenkins() {
            with_env(
                &[("JENKINS_URL", "http://jenkins.local")],
                ALL_CI_VARS,
                || {
                    assert_eq!(detect_ci_env_var(), Some("JENKINS_URL".to_string()));
                },
            );
        }

        #[test]
        fn detects_buildkite() {
            with_env(&[("BUILDKITE", "true")], ALL_CI_VARS, || {
                assert_eq!(detect_ci_env_var(), Some("BUILDKITE".to_string()));
            });
        }

        #[test]
        fn detects_azure_devops() {
            with_env(&[("TF_BUILD", "True")], ALL_CI_VARS, || {
                assert_eq!(detect_ci_env_var(), Some("TF_BUILD".to_string()));
            });
        }
    }

    mod non_interactive_reason_tests {
        use super::*;

        #[test]
        fn no_tty_takes_highest_priority() {
            with_env(
                &[
                    ("CARGO_CHANGESET_NO_TTY", "1"),
                    ("CARGO_CHANGESET_FORCE_TTY", "1"),
                    ("CI", "true"),
                ],
                ALL_CI_VARS,
                || {
                    assert_eq!(
                        non_interactive_reason(),
                        Some(NonInteractiveReason::ExplicitDisable)
                    );
                },
            );
        }

        #[test]
        fn force_tty_takes_priority_over_ci_detection() {
            with_env(
                &[("CI", "true"), ("CARGO_CHANGESET_FORCE_TTY", "1")],
                ALL_CI_VARS,
                || {
                    assert!(non_interactive_reason().is_none());
                },
            );
        }

        #[test]
        fn force_tty_allows_interactivity_when_no_ci() {
            with_env(&[("CARGO_CHANGESET_FORCE_TTY", "1")], ALL_CI_VARS, || {
                assert!(non_interactive_reason().is_none());
            });
        }

        #[test]
        fn explicit_disable_returns_correct_reason() {
            with_env(&[("CARGO_CHANGESET_NO_TTY", "1")], ALL_CI_VARS, || {
                assert_eq!(
                    non_interactive_reason(),
                    Some(NonInteractiveReason::ExplicitDisable)
                );
            });
        }

        #[test]
        fn ci_detection_returns_correct_env_var() {
            with_env(&[("GITHUB_ACTIONS", "true")], ALL_CI_VARS, || {
                assert_eq!(
                    non_interactive_reason(),
                    Some(NonInteractiveReason::CiDetected {
                        env_var: "GITHUB_ACTIONS".to_string()
                    })
                );
            });
        }
    }

    mod is_interactive_tests {
        use super::*;

        #[test]
        fn returns_false_when_no_tty_set() {
            with_env(&[("CARGO_CHANGESET_NO_TTY", "1")], ALL_CI_VARS, || {
                assert!(!is_interactive());
            });
        }

        #[test]
        fn returns_false_when_ci_detected() {
            with_env(&[("CI", "true")], ALL_CI_VARS, || {
                assert!(!is_interactive());
            });
        }

        #[test]
        fn returns_true_when_force_tty_and_no_ci() {
            with_env(&[("CARGO_CHANGESET_FORCE_TTY", "1")], ALL_CI_VARS, || {
                assert!(is_interactive());
            });
        }

        #[test]
        fn returns_true_when_force_tty_overrides_ci() {
            with_env(
                &[("CI", "true"), ("CARGO_CHANGESET_FORCE_TTY", "1")],
                ALL_CI_VARS,
                || {
                    assert!(is_interactive());
                },
            );
        }
    }
}
