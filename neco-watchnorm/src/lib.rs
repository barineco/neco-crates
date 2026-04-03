//! Host-agnostic file watcher event normalization and batch coalescing.

/// Host-level watcher kind before normalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawWatchKind {
    Create,
    Remove,
    Modify,
    Rename,
}

/// Optional rename-side hint carried by the host event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameHint {
    From,
    To,
    Both,
    Any,
}

/// Host-agnostic watcher input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawWatchEvent {
    pub kind: RawWatchKind,
    pub paths: Vec<String>,
    pub rename_from: Option<String>,
    pub rename_to: Option<String>,
    pub rename_hint: Option<RenameHint>,
    pub generation: u64,
}

/// Consumer-facing normalized watcher event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedWatchEvent {
    pub generation: u64,
    pub kind: NormalizedWatchKind,
}

/// Consumer-facing normalized watcher kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NormalizedWatchKind {
    Create {
        path: String,
    },
    Remove {
        path: String,
    },
    Modify {
        path: String,
    },
    Rename {
        from: String,
        to: String,
    },
    PartialRename {
        from: Option<String>,
        to: Option<String>,
    },
}

/// Result returned from a batch drain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlushResult {
    pub events: Vec<NormalizedWatchEvent>,
    pub discarded_stale: usize,
}

/// Stateful batch normalizer for watcher events.
#[derive(Debug, Default, Clone)]
pub struct WatchBatchNormalizer {
    pending: Vec<RawWatchEvent>,
}

impl WatchBatchNormalizer {
    /// Create an empty normalizer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a raw host event for the next drain.
    pub fn push(&mut self, event: RawWatchEvent) {
        self.pending.push(event);
    }

    /// Normalize and coalesce queued events.
    pub fn drain(&mut self, current_generation: u64) -> FlushResult {
        let pending = core::mem::take(&mut self.pending);
        let mut discarded_stale = 0usize;
        let mut normalized = Vec::new();
        let mut pending_rename_from: Option<(u64, String)> = None;

        for event in pending {
            if event.generation < current_generation {
                discarded_stale += 1;
                continue;
            }

            match event.kind {
                RawWatchKind::Create => {
                    for path in event.paths {
                        normalized.push(NormalizedWatchEvent {
                            generation: event.generation,
                            kind: NormalizedWatchKind::Create { path },
                        });
                    }
                }
                RawWatchKind::Remove => {
                    for path in event.paths {
                        normalized.push(NormalizedWatchEvent {
                            generation: event.generation,
                            kind: NormalizedWatchKind::Remove { path },
                        });
                    }
                }
                RawWatchKind::Modify => {
                    for path in event.paths {
                        normalized.push(NormalizedWatchEvent {
                            generation: event.generation,
                            kind: NormalizedWatchKind::Modify { path },
                        });
                    }
                }
                RawWatchKind::Rename => {
                    normalize_rename_event(event, &mut pending_rename_from, &mut normalized);
                }
            }
        }

        if let Some((generation, from)) = pending_rename_from.take() {
            normalized.push(NormalizedWatchEvent {
                generation,
                kind: NormalizedWatchKind::PartialRename {
                    from: Some(from),
                    to: None,
                },
            });
        }

        FlushResult {
            events: coalesce_events(normalized),
            discarded_stale,
        }
    }
}

fn normalize_rename_event(
    event: RawWatchEvent,
    pending_rename_from: &mut Option<(u64, String)>,
    normalized: &mut Vec<NormalizedWatchEvent>,
) {
    let generation = event.generation;
    let (rename_from, rename_to) = resolve_rename_paths(&event);

    match (rename_from, rename_to) {
        (Some(from), Some(to)) => {
            if let Some((pending_generation, pending_from)) = pending_rename_from.take() {
                normalized.push(NormalizedWatchEvent {
                    generation: pending_generation,
                    kind: NormalizedWatchKind::PartialRename {
                        from: Some(pending_from),
                        to: None,
                    },
                });
            }
            normalized.push(NormalizedWatchEvent {
                generation,
                kind: NormalizedWatchKind::Rename { from, to },
            });
        }
        (Some(from), None) => {
            if let Some((pending_generation, pending_from)) =
                pending_rename_from.replace((generation, from))
            {
                normalized.push(NormalizedWatchEvent {
                    generation: pending_generation,
                    kind: NormalizedWatchKind::PartialRename {
                        from: Some(pending_from),
                        to: None,
                    },
                });
            }
        }
        (None, Some(to)) => {
            if let Some((pending_generation, from)) = pending_rename_from.take() {
                if pending_generation == generation {
                    normalized.push(NormalizedWatchEvent {
                        generation,
                        kind: NormalizedWatchKind::Rename { from, to },
                    });
                } else {
                    normalized.push(NormalizedWatchEvent {
                        generation: pending_generation,
                        kind: NormalizedWatchKind::PartialRename {
                            from: Some(from),
                            to: None,
                        },
                    });
                    normalized.push(NormalizedWatchEvent {
                        generation,
                        kind: NormalizedWatchKind::PartialRename {
                            from: None,
                            to: Some(to),
                        },
                    });
                }
            } else {
                normalized.push(NormalizedWatchEvent {
                    generation,
                    kind: NormalizedWatchKind::PartialRename {
                        from: None,
                        to: Some(to),
                    },
                });
            }
        }
        (None, None) => {}
    }
}

fn resolve_rename_paths(event: &RawWatchEvent) -> (Option<String>, Option<String>) {
    let explicit_from = event.rename_from.clone();
    let explicit_to = event.rename_to.clone();
    if explicit_from.is_some() || explicit_to.is_some() {
        return (explicit_from, explicit_to);
    }

    match event.rename_hint {
        Some(RenameHint::From) => (event.paths.first().cloned(), None),
        Some(RenameHint::To) => (None, event.paths.first().cloned()),
        Some(RenameHint::Both) | Some(RenameHint::Any) | None => {
            if event.paths.len() > 1 {
                (event.paths.first().cloned(), event.paths.get(1).cloned())
            } else {
                (None, None)
            }
        }
    }
}

fn coalesce_events(events: Vec<NormalizedWatchEvent>) -> Vec<NormalizedWatchEvent> {
    let mut coalesced = Vec::with_capacity(events.len());

    for event in events {
        match &event.kind {
            NormalizedWatchKind::Modify { path } => {
                if last_is_modify_for_path(&coalesced, path) {
                    continue;
                }
                if last_is_create_for_path(&coalesced, path) {
                    continue;
                }
                coalesced.push(event);
            }
            NormalizedWatchKind::Remove { path } => {
                coalesced.retain(|item| {
                    !matches!(&item.kind, NormalizedWatchKind::Modify { path: last_path } if last_path == path)
                });
                coalesced.push(event);
            }
            _ => coalesced.push(event),
        }
    }

    coalesced
}

fn last_is_modify_for_path(events: &[NormalizedWatchEvent], path: &str) -> bool {
    matches!(
        events.last(),
        Some(NormalizedWatchEvent {
            kind: NormalizedWatchKind::Modify { path: last_path },
            ..
        }) if last_path == path
    )
}

fn last_is_create_for_path(events: &[NormalizedWatchEvent], path: &str) -> bool {
    matches!(
        events.last(),
        Some(NormalizedWatchEvent {
            kind: NormalizedWatchKind::Create { path: last_path },
            ..
        }) if last_path == path
    )
}

#[cfg(test)]
mod tests {
    use super::{
        FlushResult, NormalizedWatchEvent, NormalizedWatchKind, RawWatchEvent, RawWatchKind,
        RenameHint, WatchBatchNormalizer,
    };

    #[test]
    fn rename_both_normalizes_into_single_rename() {
        let mut normalizer = WatchBatchNormalizer::new();
        normalizer.push(raw_rename(
            4,
            vec!["/old.txt", "/new.txt"],
            None,
            None,
            Some(RenameHint::Both),
        ));

        let result = normalizer.drain(4);

        assert_eq!(
            result.events,
            vec![normalized(
                4,
                NormalizedWatchKind::Rename {
                    from: "/old.txt".into(),
                    to: "/new.txt".into(),
                },
            )]
        );
        assert_eq!(result.discarded_stale, 0);
    }

    #[test]
    fn rename_from_and_to_are_joined_within_one_drain() {
        let mut normalizer = WatchBatchNormalizer::new();
        normalizer.push(raw_rename(
            7,
            vec!["/from.txt"],
            Some("/from.txt"),
            None,
            Some(RenameHint::From),
        ));
        normalizer.push(raw_rename(
            7,
            vec!["/to.txt"],
            None,
            Some("/to.txt"),
            Some(RenameHint::To),
        ));

        let result = normalizer.drain(7);

        assert_eq!(
            result.events,
            vec![normalized(
                7,
                NormalizedWatchKind::Rename {
                    from: "/from.txt".into(),
                    to: "/to.txt".into(),
                },
            )]
        );
    }

    #[test]
    fn unresolved_rename_from_becomes_partial_rename() {
        let mut normalizer = WatchBatchNormalizer::new();
        normalizer.push(raw_rename(
            2,
            vec!["/ghost.txt"],
            Some("/ghost.txt"),
            None,
            Some(RenameHint::From),
        ));

        let result = normalizer.drain(2);

        assert_eq!(
            result.events,
            vec![normalized(
                2,
                NormalizedWatchKind::PartialRename {
                    from: Some("/ghost.txt".into()),
                    to: None,
                },
            )]
        );
    }

    #[test]
    fn rename_to_without_pending_becomes_partial_rename() {
        let mut normalizer = WatchBatchNormalizer::new();
        normalizer.push(raw_rename(
            3,
            vec!["/arrived.txt"],
            None,
            Some("/arrived.txt"),
            Some(RenameHint::To),
        ));

        let result = normalizer.drain(3);

        assert_eq!(
            result.events,
            vec![normalized(
                3,
                NormalizedWatchKind::PartialRename {
                    from: None,
                    to: Some("/arrived.txt".into()),
                },
            )]
        );
    }

    #[test]
    fn stale_generations_are_discarded() {
        let mut normalizer = WatchBatchNormalizer::new();
        normalizer.push(RawWatchEvent {
            kind: RawWatchKind::Modify,
            paths: vec!["/stale.txt".into()],
            rename_from: None,
            rename_to: None,
            rename_hint: None,
            generation: 4,
        });
        normalizer.push(RawWatchEvent {
            kind: RawWatchKind::Modify,
            paths: vec!["/fresh.txt".into()],
            rename_from: None,
            rename_to: None,
            rename_hint: None,
            generation: 5,
        });

        let result = normalizer.drain(5);

        assert_eq!(
            result,
            FlushResult {
                events: vec![normalized(
                    5,
                    NormalizedWatchKind::Modify {
                        path: "/fresh.txt".into(),
                    },
                )],
                discarded_stale: 1,
            }
        );
    }

    #[test]
    fn consecutive_modify_events_coalesce_per_path() {
        let mut normalizer = WatchBatchNormalizer::new();
        normalizer.push(raw_modify(8, "/same.txt"));
        normalizer.push(raw_modify(8, "/same.txt"));
        normalizer.push(raw_modify(8, "/other.txt"));

        let result = normalizer.drain(8);

        assert_eq!(
            result.events,
            vec![
                normalized(
                    8,
                    NormalizedWatchKind::Modify {
                        path: "/same.txt".into(),
                    },
                ),
                normalized(
                    8,
                    NormalizedWatchKind::Modify {
                        path: "/other.txt".into(),
                    },
                ),
            ]
        );
    }

    #[test]
    fn create_absorbs_immediate_modify_for_same_path() {
        let mut normalizer = WatchBatchNormalizer::new();
        normalizer.push(RawWatchEvent {
            kind: RawWatchKind::Create,
            paths: vec!["/fresh.txt".into()],
            rename_from: None,
            rename_to: None,
            rename_hint: None,
            generation: 9,
        });
        normalizer.push(raw_modify(9, "/fresh.txt"));

        let result = normalizer.drain(9);

        assert_eq!(
            result.events,
            vec![normalized(
                9,
                NormalizedWatchKind::Create {
                    path: "/fresh.txt".into(),
                },
            )]
        );
    }

    #[test]
    fn remove_drops_pending_modify_for_same_path() {
        let mut normalizer = WatchBatchNormalizer::new();
        normalizer.push(raw_modify(10, "/deleted.txt"));
        normalizer.push(RawWatchEvent {
            kind: RawWatchKind::Remove,
            paths: vec!["/deleted.txt".into()],
            rename_from: None,
            rename_to: None,
            rename_hint: None,
            generation: 10,
        });

        let result = normalizer.drain(10);

        assert_eq!(
            result.events,
            vec![normalized(
                10,
                NormalizedWatchKind::Remove {
                    path: "/deleted.txt".into(),
                },
            )]
        );
    }

    #[test]
    fn pending_rename_from_does_not_cross_drain_boundaries() {
        let mut normalizer = WatchBatchNormalizer::new();
        normalizer.push(raw_rename(
            11,
            vec!["/from.txt"],
            Some("/from.txt"),
            None,
            Some(RenameHint::From),
        ));

        let first = normalizer.drain(11);
        assert_eq!(
            first.events,
            vec![normalized(
                11,
                NormalizedWatchKind::PartialRename {
                    from: Some("/from.txt".into()),
                    to: None,
                },
            )]
        );

        normalizer.push(raw_rename(
            11,
            vec!["/to.txt"],
            None,
            Some("/to.txt"),
            Some(RenameHint::To),
        ));

        let second = normalizer.drain(11);
        assert_eq!(
            second.events,
            vec![normalized(
                11,
                NormalizedWatchKind::PartialRename {
                    from: None,
                    to: Some("/to.txt".into()),
                },
            )]
        );
    }

    #[test]
    fn pending_rename_from_does_not_join_to_different_generation_within_one_drain() {
        let mut normalizer = WatchBatchNormalizer::new();
        normalizer.push(raw_rename(
            20,
            vec!["/from.txt"],
            Some("/from.txt"),
            None,
            Some(RenameHint::From),
        ));
        normalizer.push(raw_rename(
            21,
            vec!["/to.txt"],
            None,
            Some("/to.txt"),
            Some(RenameHint::To),
        ));

        let result = normalizer.drain(20);

        assert_eq!(
            result.events,
            vec![
                normalized(
                    20,
                    NormalizedWatchKind::PartialRename {
                        from: Some("/from.txt".into()),
                        to: None,
                    },
                ),
                normalized(
                    21,
                    NormalizedWatchKind::PartialRename {
                        from: None,
                        to: Some("/to.txt".into()),
                    },
                ),
            ]
        );
    }

    #[test]
    fn completed_rename_does_not_drop_older_pending_rename_from() {
        let mut normalizer = WatchBatchNormalizer::new();
        normalizer.push(raw_rename(
            12,
            vec!["/stale-from.txt"],
            Some("/stale-from.txt"),
            None,
            Some(RenameHint::From),
        ));
        normalizer.push(raw_rename(
            12,
            vec!["/from.txt", "/to.txt"],
            Some("/from.txt"),
            Some("/to.txt"),
            Some(RenameHint::Both),
        ));

        let result = normalizer.drain(12);

        assert_eq!(
            result.events,
            vec![
                normalized(
                    12,
                    NormalizedWatchKind::PartialRename {
                        from: Some("/stale-from.txt".into()),
                        to: None,
                    },
                ),
                normalized(
                    12,
                    NormalizedWatchKind::Rename {
                        from: "/from.txt".into(),
                        to: "/to.txt".into(),
                    },
                ),
            ]
        );
    }

    fn raw_rename(
        generation: u64,
        paths: Vec<&str>,
        rename_from: Option<&str>,
        rename_to: Option<&str>,
        rename_hint: Option<RenameHint>,
    ) -> RawWatchEvent {
        RawWatchEvent {
            kind: RawWatchKind::Rename,
            paths: paths.into_iter().map(str::to_owned).collect(),
            rename_from: rename_from.map(str::to_owned),
            rename_to: rename_to.map(str::to_owned),
            rename_hint,
            generation,
        }
    }

    fn raw_modify(generation: u64, path: &str) -> RawWatchEvent {
        RawWatchEvent {
            kind: RawWatchKind::Modify,
            paths: vec![path.into()],
            rename_from: None,
            rename_to: None,
            rename_hint: None,
            generation,
        }
    }

    fn normalized(generation: u64, kind: NormalizedWatchKind) -> NormalizedWatchEvent {
        NormalizedWatchEvent { generation, kind }
    }
}
