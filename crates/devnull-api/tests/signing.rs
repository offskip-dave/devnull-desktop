use std::process::Command;

use devnull_api::{target, DeviceKey, JSON_NAMESPACE, MULTIPART_NAMESPACE};

#[test]
fn fingerprint_has_authorize_format() {
    let key = DeviceKey::generate();
    let fp = key.fingerprint();
    assert!(fp.starts_with("SHA256:"));
    let rest = &fp["SHA256:".len()..];
    assert_eq!(rest.len(), 43);
    assert!(rest
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/'));
}

#[test]
fn fingerprint_matches_ssh_keygen() {
    if !ssh_keygen_available() {
        return;
    }
    let key = DeviceKey::generate();
    let dir = tempfile();
    let pub_path = dir.join("key.pub");
    std::fs::write(&pub_path, format!("{}\n", key.openssh_public())).unwrap();

    let output = Command::new("ssh-keygen")
        .args(["-l", "-E", "sha256", "-f"])
        .arg(&pub_path)
        .output()
        .expect("ssh-keygen");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&key.fingerprint()),
        "ssh-keygen output: {stdout}"
    );
}

#[test]
fn signature_verifies_with_ssh_keygen() {
    if !ssh_keygen_available() {
        return;
    }
    for namespace in [JSON_NAMESPACE, MULTIPART_NAMESPACE] {
        let key = DeviceKey::generate();
        let message = b"1730000000\n0123456789abcdef0123456789abcdef\nPOST\n/api/v1/remotes/@nunomaduro/messages\n{\"body\":\"hi\"}";
        let signature = key.sign(message, namespace);

        let dir = tempfile();
        let sig_path = dir.join("message.sig");
        let armored = armor(&signature);
        std::fs::write(&sig_path, armored).unwrap();
        let msg_path = dir.join("message");
        std::fs::write(&msg_path, message).unwrap();

        let output = Command::new("ssh-keygen")
            .args(["-Y", "check-novalidate", "-n", namespace, "-s"])
            .arg(&sig_path)
            .stdin(std::fs::File::open(&msg_path).unwrap())
            .output()
            .expect("ssh-keygen");
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            combined.contains(&format!("Good \"{namespace}\" signature")),
            "namespace={namespace} out={combined}"
        );
    }
}

#[test]
fn target_keeps_path_and_sorted_encoded_query() {
    assert_eq!(
        target(
            "remotes/@nuno/messages",
            &[("reactions_after", Some("3")), ("after", Some("10"))]
        ),
        "/api/v1/remotes/@nuno/messages?after=10&reactions_after=3"
    );
    assert_eq!(
        target("remotes", &[("q", Some("a b&c")), ("tab", None)]),
        "/api/v1/remotes?q=a%20b%26c"
    );
}

#[test]
fn public_user_json_decodes() {
    let json = r#"{"data":{"id":"01a13fa8-6cd4-70ef-b8e5-f46a3ca70ca8","username":"nunomaduro","name":"nunomaduro","avatar_url":"https://avatars.githubusercontent.com/u/5457236?v=4","bio":"x","company":"@laravel","location":"Portugal","website":"https://youtube.com/@nunomaduro","github_url":"https://github.com/nunomaduro","html_url":"https://dev-null.sh/@nunomaduro","feed_url":"https://dev-null.sh/@nunomaduro/feed.atom","created_at":"2026-10-15T13:02:44Z","commits_count":355,"followers_count":5470,"following_count":120}}"#;
    let env: devnull_api::DataEnvelope<devnull_api::User> = serde_json::from_str(json).unwrap();
    assert_eq!(env.data.username, "nunomaduro");
    assert_eq!(env.data.commits_count, Some(355));
}

#[test]
fn commit_page_with_youtube_embed_decodes() {
    let json = r#"{
        "data": [{
            "id": "01a16ac5-19d9-71e7-a44e-36b6d5f006bc",
            "short_hash": "5f006bc",
            "body": "watch this",
            "parent_id": null,
            "html_url": "https://dev-null.sh/@nunomaduro/commits/01a16ac5-19d9-71e7-a44e-36b6d5f006bc",
            "created_at": "2026-10-23T21:57:44Z",
            "stars_count": 32,
            "replies_count": 4,
            "link_preview": {
                "url": "https://youtu.be/5X5csEbmHqs",
                "title": "YouTube",
                "description": null,
                "image_url": null,
                "site_name": null,
                "is_pending": false,
                "embed": {
                    "provider": "YouTube",
                    "kind": "video",
                    "src": "https://www.youtube-nocookie.com/embed/5X5csEbmHqs?autoplay=1",
                    "aspect_ratio": "16 / 9",
                    "height": null,
                    "poster": "https://i.ytimg.com/vi/5X5csEbmHqs/hqdefault.jpg"
                }
            },
            "images": [],
            "author": {"id": "01a13fa8-6cd4-70ef-b8e5-f46a3ca70ca8", "username": "nunomaduro"}
        }],
        "links": {"next": null},
        "meta": {"next_cursor": null}
    }"#;
    let page: devnull_api::Page<devnull_api::Commit> = serde_json::from_str(json).unwrap();
    assert_eq!(page.data[0].author.as_ref().unwrap().username, "nunomaduro");
    assert!(page.data[0]
        .link_preview
        .as_ref()
        .unwrap()
        .embed
        .as_ref()
        .unwrap()
        .src
        .is_some());
}

#[test]
fn commit_detail_page_reads_ancestors() {
    let json = r#"{
        "data": {
            "commit": {
                "id": "child",
                "body": "a reply",
                "parent_id": "parent",
                "author": {"id": "u2", "username": "ada"}
            },
            "ancestors": [
                {"id": "parent", "body": "root", "author": {"id": "u1", "username": "grace"}}
            ]
        }
    }"#;
    let page = devnull_api::CommitPage::from_value(serde_json::from_str(json).unwrap()).unwrap();
    assert_eq!(page.commit.id, "child");
    assert_eq!(page.ancestors.len(), 1);
    assert_eq!(page.ancestors[0].author.as_ref().unwrap().username, "grace");
}

#[test]
fn commit_and_message_mentions_decode() {
    let commit: devnull_api::Commit = serde_json::from_str(
        r#"{
            "id": "c1",
            "body": "Thanks @ORIGINAL!",
            "mentions": {"original": "019a2b3c-4d5e-7f60-8a9b-0c1d2e3f4a5b"}
        }"#,
    )
    .unwrap();
    assert_eq!(
        commit.mentions.as_ref().unwrap().get("original").unwrap(),
        "019a2b3c-4d5e-7f60-8a9b-0c1d2e3f4a5b"
    );
    let none: devnull_api::Commit =
        serde_json::from_str(r#"{"id": "c2", "body": "plain", "mentions": null}"#).unwrap();
    assert!(none.mentions.is_none());
    let message: devnull_api::Message =
        serde_json::from_str(r#"{"id": 1, "body": "Hi @nuno", "mentions": {"nuno": "u1"}}"#)
            .unwrap();
    assert_eq!(message.mentions.unwrap().get("nuno").unwrap(), "u1");
    let empty: devnull_api::Commit =
        serde_json::from_str(r#"{"id": "c3", "mentions": []}"#).unwrap();
    assert!(empty.mentions.is_none());
}

#[test]
fn notification_page_reads_groups_and_skips_broken_commits() {
    let page = devnull_api::Notification::page_from_value(serde_json::json!({
        "notifications": {
            "data": [
                {
                    "id": "n1",
                    "type": "followed",
                    "actor": {"id": "u1", "username": "ada"},
                    "actors": null,
                    "commit": null
                },
                {
                    "id": "n2",
                    "type": "replied",
                    "actor": {"id": "u2", "handle": "grace"},
                    "commit": {
                        "id": "c1",
                        "body": "hello",
                        "mentions": []
                    }
                },
                {
                    "id": "n3",
                    "type": "starred",
                    "actor": {"id": "u3", "username": "linus"},
                    "commit": "not-an-object"
                }
            ]
        }
    }));
    assert_eq!(page.data.len(), 3);
    assert_eq!(page.data[0].kind(), "followed");
    assert_eq!(
        page.data[1].commit.as_ref().unwrap().body.as_deref(),
        Some("hello")
    );
    assert_eq!(page.data[1].actor.as_ref().unwrap().username, "grace");
    assert!(page.data[2].commit.is_none());
}

#[test]
fn profile_reads_activity_achievements_and_follow_state() {
    let json = r#"{
        "data": {
            "id": "u1",
            "username": "ada",
            "name": "Ada",
            "lane": 2,
            "commits_count": 12,
            "followers_count": 4,
            "following_count": 9,
            "is_following": true,
            "follows_you": true,
            "activity": {
                "scale": "week",
                "buckets": [
                    {"date": "2026-10-20", "count": 0},
                    {"date": "2026-10-21", "count": 3}
                ]
            },
            "streaks": {"longest": 11, "current": 2},
            "achievements": {
                "level": 2,
                "rank": "user",
                "xp": 120,
                "next_level_xp": 250,
                "level_progress": 40,
                "ranks": [
                    {"rank": "guest", "level": 1, "min_xp": 0, "achieved": true},
                    {"rank": "user", "level": 2, "min_xp": 50, "achieved": true},
                    {"rank": "operator", "level": 3, "min_xp": 250, "achieved": false}
                ],
                "badges": [
                    {"id": "init", "name": "init", "unlocked": true, "progress": 1, "target": 1},
                    {"id": "uptime", "name": "uptime", "unlocked": false, "progress": 2, "target": 7}
                ]
            },
            "pasteis_received": 3
        }
    }"#;
    let profile = devnull_api::Profile::from_value(serde_json::from_str(json).unwrap()).unwrap();
    assert_eq!(profile.user.username, "ada");
    assert!(profile.is_following);
    assert!(profile.follows_you);
    assert_eq!(profile.streaks.current, 2);
    assert_eq!(profile.activity.cells().len(), 2);
    assert_eq!(profile.activity.cells()[1].level, 4);
    let achievements = profile.achievements.unwrap();
    assert_eq!(achievements.label(), "Lv. 2 · user");
    assert_eq!(achievements.xp_label(), "120 / 250 XP");
    assert_eq!(achievements.next_rank_label(), "130 XP to operator");
    assert_eq!(achievements.unlocked_count(), 1);
    assert_eq!(
        achievements
            .unlocked_badges()
            .map(|badge| badge.id.as_str())
            .collect::<Vec<_>>(),
        ["init"]
    );
    assert_eq!(profile.pasteis_received, Some(3));
}

fn armor(signature: &str) -> String {
    let mut out = String::from("-----BEGIN SSH SIGNATURE-----\n");
    for chunk in signature.as_bytes().chunks(70) {
        out.push_str(std::str::from_utf8(chunk).unwrap());
        out.push('\n');
    }
    out.push_str("-----END SSH SIGNATURE-----\n");
    out
}

fn ssh_keygen_available() -> bool {
    Command::new("ssh-keygen").arg("-V").output().is_ok()
}

fn tempfile() -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("devnull-sign-{nanos}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
