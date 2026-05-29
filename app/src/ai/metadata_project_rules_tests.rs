use std::collections::HashMap;
use std::fs;

use ai::project_context::model::{ProjectContextModel, ProjectContextModelEvent};
use remote_server::proto::{
    ProjectContextFile, ProjectContextFileKind, ProjectContextFilesSnapshot,
};
use repo_metadata::RepositoryIdentifier;
use tempfile::TempDir;
use warp_util::host_id::HostId;
use warp_util::local_or_remote_path::LocalOrRemotePath;
use warp_util::remote_path::RemotePath;
use warp_util::standardized_path::StandardizedPath;
use warpui::{App, Entity, ModelContext, SingletonEntity};

use super::MetadataProjectRulesModel;

struct RulesIndexedListener;

impl RulesIndexedListener {
    fn new(indexed_tx: async_channel::Sender<()>, ctx: &mut ModelContext<Self>) -> Self {
        ctx.subscribe_to_model(&ProjectContextModel::handle(ctx), move |_, event, _| {
            if matches!(event, ProjectContextModelEvent::PathIndexed) {
                let _ = indexed_tx.try_send(());
            }
        });
        Self
    }
}

impl Entity for RulesIndexedListener {
    type Event = ();
}

struct RulesDeltaListener;

impl RulesDeltaListener {
    fn new(
        deleted_tx: async_channel::Sender<Vec<std::path::PathBuf>>,
        ctx: &mut ModelContext<Self>,
    ) -> Self {
        ctx.subscribe_to_model(&ProjectContextModel::handle(ctx), move |_, event, _| {
            if let ProjectContextModelEvent::KnownRulesChanged(delta) = event {
                let _ = deleted_tx.try_send(delta.deleted_rules.clone());
            }
        });
        Self
    }
}

impl Entity for RulesDeltaListener {
    type Event = ();
}

fn metadata_rules_model() -> MetadataProjectRulesModel {
    MetadataProjectRulesModel {
        refresh_generations: HashMap::new(),
        next_refresh_generation: 0,
    }
}

fn remote_rule_path(host_id: &HostId, path: &str) -> LocalOrRemotePath {
    LocalOrRemotePath::Remote(RemotePath::new(
        host_id.clone(),
        StandardizedPath::try_new(path).unwrap(),
    ))
}

#[test]
fn local_metadata_refresh_uses_dedicated_rule_scan() {
    let (indexed_tx, indexed_rx) = async_channel::unbounded();

    App::test((), |mut app| async move {
        let project_context = app.add_singleton_model(|_| ProjectContextModel::default());
        let _listener = app.add_model(|ctx| RulesIndexedListener::new(indexed_tx, ctx));
        let rules_model = app.add_model(|_| metadata_rules_model());

        let temp_dir = TempDir::new().unwrap();
        let repo = dunce::canonicalize(temp_dir.path()).unwrap();
        let nested_dir = repo.join("src");
        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(repo.join("WARP.md"), "root rule").unwrap();
        fs::write(nested_dir.join("WARP.md"), "nested rule").unwrap();
        let repo_id = RepositoryIdentifier::try_local(&repo).unwrap();

        rules_model.update(&mut app, |model, ctx| {
            model.refresh_local_project_rules(&repo_id, ctx);
        });
        indexed_rx.recv().await.unwrap();

        project_context.read(&app, |model, _| {
            let result = model
                .find_applicable_project_rules(&nested_dir.join("main.rs"))
                .expect("dedicated metadata-triggered scan should discover rules");
            assert_eq!(result.active_rules.len(), 2);
            let contents: Vec<&str> = result
                .active_rules
                .iter()
                .map(|rule| rule.content.as_str())
                .collect();
            assert!(contents.contains(&"root rule"));
            assert!(contents.contains(&"nested rule"));
        });
    });
}

#[test]
fn removed_local_metadata_repository_clears_rules_and_persists_deletion() {
    let (indexed_tx, indexed_rx) = async_channel::unbounded();
    let (deleted_tx, deleted_rx) = async_channel::unbounded();

    App::test((), |mut app| async move {
        let project_context = app.add_singleton_model(|_| ProjectContextModel::default());
        let _indexed_listener = app.add_model(|ctx| RulesIndexedListener::new(indexed_tx, ctx));
        let _deleted_listener = app.add_model(|ctx| RulesDeltaListener::new(deleted_tx, ctx));
        let rules_model = app.add_model(|_| metadata_rules_model());

        let temp_dir = TempDir::new().unwrap();
        let repo = dunce::canonicalize(temp_dir.path()).unwrap();
        let rule_path = repo.join("WARP.md");
        fs::write(&rule_path, "metadata rule").unwrap();
        let repo_id = RepositoryIdentifier::try_local(&repo).unwrap();

        rules_model.update(&mut app, |model, ctx| {
            model.refresh_local_project_rules(&repo_id, ctx);
        });
        indexed_rx.recv().await.unwrap();
        assert!(deleted_rx.recv().await.unwrap().is_empty());

        rules_model.update(&mut app, |model, ctx| {
            model.clear_project_rules_for_removed_repository(&repo_id, ctx);
        });
        indexed_rx.recv().await.unwrap();
        assert_eq!(deleted_rx.recv().await.unwrap(), vec![rule_path]);

        project_context.read(&app, |model, _| {
            assert!(model
                .find_applicable_project_rules(&repo.join("src/main.rs"))
                .is_none());
        });
    });
}

#[test]
fn remote_project_rule_snapshot_replaces_removed_files() {
    App::test((), |mut app| async move {
        let project_context = app.add_singleton_model(|_| ProjectContextModel::default());
        let rules_model = app.add_model(|_| metadata_rules_model());
        let host = HostId::new("test-host".to_string());
        let nested_path = remote_rule_path(&host, "/repo/nested/WARP.md");

        rules_model.update(&mut app, |model, ctx| {
            model.handle_remote_project_context_snapshot(
                &host,
                &ProjectContextFilesSnapshot {
                    repo_path: "/repo".to_string(),
                    kind: ProjectContextFileKind::ProjectRules.into(),
                    files: vec![ProjectContextFile {
                        path: "/repo/nested/WARP.md".to_string(),
                        content: "remote rules".to_string(),
                    }],
                },
                ctx,
            );
        });
        project_context.read(&app, |model, _| {
            let result = model
                .find_applicable_project_rules_at_location(&remote_rule_path(
                    &host,
                    "/repo/nested/main.rs",
                ))
                .expect("typed remote snapshot should hydrate project rules");
            assert_eq!(result.active_rules[0].path, nested_path);
            assert_eq!(result.active_rules[0].content, "remote rules");
        });

        rules_model.update(&mut app, |model, ctx| {
            model.handle_remote_project_context_snapshot(
                &host,
                &ProjectContextFilesSnapshot {
                    repo_path: "/repo".to_string(),
                    kind: ProjectContextFileKind::ProjectRules.into(),
                    files: vec![],
                },
                ctx,
            );
        });
        project_context.read(&app, |model, _| {
            assert!(model
                .find_applicable_project_rules_at_location(&remote_rule_path(
                    &host,
                    "/repo/nested/main.rs",
                ))
                .is_none());
        });
    });
}

#[test]
fn non_rule_project_context_snapshot_is_ignored() {
    App::test((), |mut app| async move {
        let project_context = app.add_singleton_model(|_| ProjectContextModel::default());
        let rules_model = app.add_model(|_| metadata_rules_model());
        let host = HostId::new("test-host".to_string());

        rules_model.update(&mut app, |model, ctx| {
            model.handle_remote_project_context_snapshot(
                &host,
                &ProjectContextFilesSnapshot {
                    repo_path: "/repo".to_string(),
                    kind: ProjectContextFileKind::ProjectSkills.into(),
                    files: vec![ProjectContextFile {
                        path: "/repo/WARP.md".to_string(),
                        content: "not a rules snapshot".to_string(),
                    }],
                },
                ctx,
            );
        });
        project_context.read(&app, |model, _| {
            assert!(model
                .find_applicable_project_rules_at_location(&remote_rule_path(
                    &host,
                    "/repo/main.rs",
                ))
                .is_none());
        });
    });
}

#[test]
fn host_disconnect_clears_remote_project_rules() {
    App::test((), |mut app| async move {
        let project_context = app.add_singleton_model(|_| ProjectContextModel::default());
        let rules_model = app.add_model(|_| metadata_rules_model());
        let host = HostId::new("test-host".to_string());

        rules_model.update(&mut app, |model, ctx| {
            model.handle_remote_project_context_snapshot(
                &host,
                &ProjectContextFilesSnapshot {
                    repo_path: "/repo".to_string(),
                    kind: ProjectContextFileKind::ProjectRules.into(),
                    files: vec![ProjectContextFile {
                        path: "/repo/WARP.md".to_string(),
                        content: "remote rules".to_string(),
                    }],
                },
                ctx,
            );
            model.remove_remote_project_rules_for_host(&host, ctx);
        });
        project_context.read(&app, |model, _| {
            assert!(model
                .find_applicable_project_rules_at_location(&remote_rule_path(
                    &host,
                    "/repo/main.rs",
                ))
                .is_none());
        });
    });
}
