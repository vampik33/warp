use std::collections::HashMap;

use ai::project_context::model::{ProjectContextModel, ProjectRule};
use remote_server::manager::{RemoteServerManager, RemoteServerManagerEvent};
use remote_server::proto::{ProjectContextFileKind, ProjectContextFilesSnapshot};
use repo_metadata::{RepoMetadataModel, RepositoryIdentifier};
use warp_util::host_id::HostId;
use warp_util::local_or_remote_path::LocalOrRemotePath;
use warp_util::remote_path::RemotePath;
use warp_util::standardized_path::StandardizedPath;
use warpui::{Entity, ModelContext, SingletonEntity};

pub(crate) struct MetadataProjectRulesModel {
    refresh_generations: HashMap<RepositoryIdentifier, u64>,
    next_refresh_generation: u64,
}

impl MetadataProjectRulesModel {
    pub(crate) fn new(ctx: &mut ModelContext<Self>) -> Self {
        let repo_metadata = RepoMetadataModel::handle(ctx);
        ctx.subscribe_to_model(&repo_metadata, |me, event, ctx| {
            me.handle_repo_metadata_event(event, ctx);
        });
        ctx.subscribe_to_model(
            &RemoteServerManager::handle(ctx),
            |me, event, ctx| match event {
                RemoteServerManagerEvent::ProjectContextFilesSnapshot { host_id, snapshot } => {
                    me.handle_remote_project_context_snapshot(host_id, snapshot, ctx);
                }
                RemoteServerManagerEvent::HostDisconnected { host_id } => {
                    me.remove_remote_project_rules_for_host(host_id, ctx);
                }
                _ => {}
            },
        );

        let mut model = Self {
            refresh_generations: HashMap::new(),
            next_refresh_generation: 0,
        };
        for repo_id in RepoMetadataModel::as_ref(ctx).local_repository_ids(ctx) {
            model.refresh_local_project_rules(&repo_id, ctx);
        }
        model
    }

    fn handle_repo_metadata_event(
        &mut self,
        event: &repo_metadata::wrapper_model::RepoMetadataEvent,
        ctx: &mut ModelContext<Self>,
    ) {
        use repo_metadata::wrapper_model::RepoMetadataEvent;

        match event {
            RepoMetadataEvent::RepositoryUpdated {
                id: repo_id @ RepositoryIdentifier::Local(_),
            }
            | RepoMetadataEvent::FileTreeEntryUpdated {
                id: repo_id @ RepositoryIdentifier::Local(_),
            }
            | RepoMetadataEvent::UpdatingRepositoryFailed {
                id: repo_id @ RepositoryIdentifier::Local(_),
            } => self.refresh_local_project_rules(repo_id, ctx),
            RepoMetadataEvent::RepositoryRemoved {
                id: repo_id @ RepositoryIdentifier::Local(_),
            } => self.clear_project_rules_for_removed_repository(repo_id, ctx),
            RepoMetadataEvent::RepositoryUpdated {
                id: RepositoryIdentifier::Remote(_),
            }
            | RepoMetadataEvent::RepositoryRemoved {
                id: RepositoryIdentifier::Remote(_),
            }
            | RepoMetadataEvent::FileTreeUpdated { .. }
            | RepoMetadataEvent::FileTreeEntryUpdated {
                id: RepositoryIdentifier::Remote(_),
            }
            | RepoMetadataEvent::UpdatingRepositoryFailed {
                id: RepositoryIdentifier::Remote(_),
            }
            | RepoMetadataEvent::IncrementalUpdateReady { .. } => {}
        }
    }

    fn refresh_local_project_rules(
        &mut self,
        repo_id: &RepositoryIdentifier,
        ctx: &mut ModelContext<Self>,
    ) {
        let refresh_generation = self.advance_refresh_generation(repo_id);
        let repo_id = repo_id.clone();
        let Some(local_root) = repo_id.local_path_buf() else {
            return;
        };
        let local_root_for_scan = local_root.clone();

        ctx.spawn(
            async move {
                ProjectContextModel::read_project_rules_from_metadata_root(&local_root_for_scan)
                    .await
            },
            move |me, rules, ctx| {
                if me.refresh_generations.get(&repo_id) != Some(&refresh_generation) {
                    return;
                }
                match rules {
                    Ok(rules) => {
                        ProjectContextModel::handle(ctx).update(ctx, |project_context, ctx| {
                            project_context
                                .replace_local_project_rules_from_metadata(local_root, rules, ctx);
                        })
                    }
                    Err(error) => {
                        log::warn!("Failed to refresh local project rules: {error:#}");
                    }
                }
            },
        );
    }

    fn handle_remote_project_context_snapshot(
        &mut self,
        host_id: &HostId,
        snapshot: &ProjectContextFilesSnapshot,
        ctx: &mut ModelContext<Self>,
    ) {
        if snapshot.kind != ProjectContextFileKind::ProjectRules as i32 {
            return;
        }
        let Ok(repo_path) = StandardizedPath::try_new(&snapshot.repo_path) else {
            log::warn!(
                "Ignoring remote project-rule snapshot with invalid repository path: {}",
                snapshot.repo_path
            );
            return;
        };
        let remote_root = RemotePath::new(host_id.clone(), repo_path);
        let repo_id = RepositoryIdentifier::Remote(remote_root.clone());
        let refresh_generation = self.advance_refresh_generation(&repo_id);
        let rules = snapshot
            .files
            .iter()
            .filter_map(|file| {
                let path = StandardizedPath::try_new(&file.path).ok()?;
                Some(ProjectRule {
                    path: LocalOrRemotePath::Remote(RemotePath::new(host_id.clone(), path)),
                    content: file.content.clone(),
                })
            })
            .collect();
        if self.refresh_generations.get(&repo_id) != Some(&refresh_generation) {
            return;
        }
        ProjectContextModel::handle(ctx).update(ctx, |model, ctx| {
            model.replace_remote_project_rules_from_metadata(remote_root, rules, ctx);
        });
    }

    fn remove_remote_project_rules_for_host(
        &mut self,
        host_id: &HostId,
        ctx: &mut ModelContext<Self>,
    ) {
        self.refresh_generations.retain(|repo_id, _| {
            !matches!(
                repo_id,
                RepositoryIdentifier::Remote(path) if path.host_id == *host_id
            )
        });
        ProjectContextModel::handle(ctx).update(ctx, |model, ctx| {
            model.clear_remote_project_rules_for_host(host_id, ctx);
        });
    }

    fn clear_project_rules_for_removed_repository(
        &mut self,
        repo_id: &RepositoryIdentifier,
        ctx: &mut ModelContext<Self>,
    ) {
        self.refresh_generations.remove(repo_id);
        let Some(local_root) = repo_id.local_path_buf() else {
            return;
        };
        ProjectContextModel::handle(ctx).update(ctx, |model, ctx| {
            model.clear_local_project_rules_for_removed_metadata_root(local_root, ctx);
        });
    }

    fn advance_refresh_generation(&mut self, repo_id: &RepositoryIdentifier) -> u64 {
        self.next_refresh_generation += 1;
        self.refresh_generations
            .insert(repo_id.clone(), self.next_refresh_generation);
        self.next_refresh_generation
    }
}

impl Entity for MetadataProjectRulesModel {
    type Event = ();
}

impl SingletonEntity for MetadataProjectRulesModel {}

#[cfg(test)]
#[path = "metadata_project_rules_tests.rs"]
mod tests;
