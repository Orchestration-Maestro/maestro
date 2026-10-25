//! Extension headers and footers replacing the built-in ones through the retained host.
use super::extension_ui::port;
use super::extension_widgets::with_factories;
use super::{Setup, World, isolated};
use crate::interactive_mode::startup::ExpandableText;
use maestro_app::{
    ComponentFactory, ExtensionAPI, ExtensionComponent, ExtensionFactory, ExtensionUIContextPort,
    FooterFactory, Local, ReadonlyFooterDataProvider, strip_ansi,
};
use maestro_tui::{Component, Text, tui::ComponentHandle};
use std::{cell::RefCell, path::Path, rc::Rc};

/// Recorded callback effects in call order.
type Trace = Rc<RefCell<Vec<String>>>;

/// The headers and footers one witness installs.
enum Replacement {
    /// A header factory.
    Header(ComponentFactory),
    /// A footer factory.
    Footer(FooterFactory),
}

/// A traced text component, with a traced disposal and, for `expandable`, a traced expansion.
fn traced(
    api: &ExtensionAPI<Local>,
    label: &'static str,
    trace: &Trace,
    expandable: bool,
) -> ExtensionComponent {
    let (disposed, expanded) = (trace.clone(), trace.clone());
    ExtensionComponent {
        component: Rc::new(Text::new(label.into(), 0, 0, None)),
        dispose: Some(api.ui_callback(move |(), _| {
            disposed.borrow_mut().push(format!("{label}:dispose"));
            Ok(())
        })),
        set_expanded: expandable.then(|| {
            api.ui_callback(move |value: bool, _| {
                expanded
                    .borrow_mut()
                    .push(format!("{label}:expanded:{value}"));
                Ok(())
            })
        }),
        width: None,
    }
}

/// A header factory recording its call before building `label`.
fn header(api: &ExtensionAPI<Local>, label: &'static str, trace: &Trace) -> ComponentFactory {
    let (api_copy, recorded) = (api.clone(), trace.clone());
    api.ui_callback(move |_, _| {
        recorded.borrow_mut().push(format!("factory:{label}"));
        Ok(traced(&api_copy, label, &recorded, true))
    })
}

/// A footer factory recording its call before building `label`.
fn footer(api: &ExtensionAPI<Local>, label: &'static str, trace: &Trace) -> FooterFactory {
    let (api_copy, recorded) = (api.clone(), trace.clone());
    api.ui_callback(move |_, _| {
        recorded.borrow_mut().push(format!("factory:{label}"));
        Ok(traced(&api_copy, label, &recorded, false))
    })
}

/// Whether two handles name one allocation.
fn same<A: ?Sized, B: ?Sized>(left: &Rc<A>, right: &Rc<B>) -> bool {
    std::ptr::addr_eq(Rc::as_ptr(left), Rc::as_ptr(right))
}

/// The built-in header's position in the header container, when it is there.
fn built_in_position(world: &World) -> Option<usize> {
    let built_in = world.mode.built_in_header.borrow().clone()?;
    let children = world.mode.header_container.children();
    let children = children.borrow();
    children.iter().position(|child| same(child, &built_in))
}

/// The header container's children rendered at the recording width, without styles.
fn header_text(world: &World) -> Vec<String> {
    world
        .mode
        .header_container
        .children()
        .borrow()
        .iter()
        .flat_map(|child| child.render(80))
        .map(|row| strip_ansi(row.trim_end()).unwrap())
        .collect()
}

/// The last component the writer renders.
fn last_root(world: &World) -> ComponentHandle {
    world.mode.ui.children().borrow().last().cloned().unwrap()
}

/// The rows of the last component the writer renders, without trailing padding.
fn footer_text(world: &World) -> Vec<String> {
    let rows = last_root(world).render(80);
    rows.iter().map(|row| row.trim_end().to_owned()).collect()
}

#[test]
fn replacements_dispose_and_restore_original_components() {
    isolated(
        "replacements_dispose_and_restore_original_components",
        async |directory| {
            let trace = Trace::default();
            let recorded = trace.clone();
            let (world, factories) = with_factories(directory, move |api| {
                vec![
                    Replacement::Header(header(api, "one", &recorded)),
                    Replacement::Header(header(api, "two", &recorded)),
                    Replacement::Footer(footer(api, "f1", &recorded)),
                    Replacement::Footer(footer(api, "f2", &recorded)),
                ]
            })
            .await;
            let ui = port(&world.mode);
            let position = built_in_position(&world).unwrap();
            let built_in_footer: ComponentHandle = world.mode.footer.clone();
            assert!(same(&last_root(&world), &built_in_footer));
            for factory in factories {
                world.runtime.settle().unwrap();
                install(&ui, factory).unwrap();
                assert!(world.runtime.pending() > 0);
            }
            assert_eq!(header_text(&world)[position], "two");
            assert_eq!(footer_text(&world), ["f2"]);
            let roots = world.mode.ui.children().borrow().len();
            ui.set_header(None).unwrap();
            ui.set_footer(None).unwrap();
            assert_eq!(built_in_position(&world), Some(position));
            assert!(same(&last_root(&world), &built_in_footer));
            assert_eq!(world.mode.ui.children().borrow().len(), roots);
            assert_eq!(
                *trace.borrow(),
                [
                    "factory:one",
                    "one:expanded:false",
                    "one:dispose",
                    "factory:two",
                    "two:expanded:false",
                    "factory:f1",
                    "f1:dispose",
                    "factory:f2",
                    "two:dispose",
                    "f2:dispose",
                ]
            );
        },
    );
}

/// The terminal, theme and metadata operands a factory received.
type Operands = Rc<RefCell<Vec<(maestro_tui::TUI, maestro_theme::LiveTheme)>>>;

#[test]
fn replacement_factories_receive_the_live_terminal_theme_and_metadata() {
    isolated(
        "replacement_factories_receive_the_live_terminal_theme_and_metadata",
        async |directory| {
            let operands = Operands::default();
            let metadata = Rc::new(RefCell::new(None::<Rc<dyn ReadonlyFooterDataProvider>>));
            let (seen, provided) = (operands.clone(), metadata.clone());
            let (world, factories) = with_factories(directory, move |api| {
                let (header_seen, footer_seen) = (seen.clone(), seen.clone());
                let provided = provided.clone();
                vec![
                    Replacement::Header(api.ui_callback(move |(ui, theme), _| {
                        header_seen.borrow_mut().push((ui, theme));
                        Ok(plain("header"))
                    })),
                    Replacement::Footer(api.ui_callback(move |(ui, theme, data), _| {
                        footer_seen.borrow_mut().push((ui, theme));
                        provided.replace(Some(data));
                        Ok(plain("footer"))
                    })),
                ]
            })
            .await;
            let ui = port(&world.mode);
            for factory in factories {
                install(&ui, factory).unwrap();
            }
            for (terminal, theme) in operands.borrow().iter() {
                assert!(Rc::ptr_eq(&terminal.children(), &world.mode.ui.children()));
                assert!(Rc::ptr_eq(
                    &theme.get().unwrap(),
                    &world.themes.theme().get().unwrap()
                ));
            }
            assert_eq!(operands.borrow().len(), 2);
            let data = metadata.take().unwrap();
            ui.set_status("key".into(), Some("text".into())).unwrap();
            assert_eq!(
                data.get_extension_statuses().iter().collect::<Vec<_>>(),
                [("key".to_owned(), "text".to_owned())]
            );
        },
    );
}

/// A plain component with no disposal, expansion or width.
fn plain(label: &str) -> ExtensionComponent {
    ExtensionComponent {
        component: Rc::new(Text::new(label.into(), 0, 0, None)),
        dispose: None,
        set_expanded: None,
        width: None,
    }
}

/// Loads `build`'s callbacks into a mode that has not started.
fn unstarted<T: 'static>(
    directory: &Path,
    build: impl Fn(&ExtensionAPI<Local>) -> T + 'static,
) -> (World, T) {
    let stored = Rc::new(RefCell::new(None));
    let output = stored.clone();
    let factory: ExtensionFactory<Local> = Rc::new(move |api| {
        output.replace(Some(build(&api)));
        Box::pin(std::future::ready(Ok(())))
    });
    let world = Setup {
        extension: Some(factory),
        ..Setup::default()
    }
    .build(directory);
    let callbacks = stored.take().unwrap();
    (world, callbacks)
}

#[test]
fn header_initialization_and_absent_position() {
    isolated(
        "header_initialization_and_absent_position",
        async |directory| {
            let trace = Trace::default();
            let recorded = trace.clone();
            let (world, early) = unstarted(directory, move |api| header(api, "early", &recorded));
            ExtensionUIContextPort::set_header(&*world.mode, Some(early)).unwrap();
            assert!(trace.borrow().is_empty());
            assert_eq!(world.runtime.pending(), 0);
            drop(world);
            let recorded = trace.clone();
            let (world, factories) =
                with_factories(directory, move |api| vec![header(api, "NEW", &recorded)]).await;
            let ui = port(&world.mode);
            world.mode.header_container.clear();
            ui.set_header(factories.into_iter().next()).unwrap();
            assert_eq!(header_text(&world), ["NEW"]);
            world.mode.header_container.clear();
            ui.set_header(None).unwrap();
            assert!(header_text(&world).is_empty());
            assert_eq!(
                *trace.borrow(),
                ["factory:NEW", "NEW:expanded:false", "NEW:dispose"]
            );
        },
    );
}

/// An old component and a replacement for `side`, one of whose phases fails with `error`.
fn failing(
    api: &ExtensionAPI<Local>,
    (side, phase): (&'static str, &'static str),
    trace: &Trace,
    error: &std::sync::Arc<()>,
) -> Vec<Replacement> {
    let (disposed, failure) = (trace.clone(), error.clone());
    let old = Rc::new(RefCell::new(Some(ExtensionComponent {
        dispose: Some(api.ui_callback(move |(), _| {
            disposed.borrow_mut().push("dispose-old".into());
            if phase == "dispose" {
                return Err(Box::new(Original(failure.clone())));
            }
            Ok(())
        })),
        ..plain("OLD")
    })));
    let (called, failure) = (trace.clone(), error.clone());
    let fail = move || -> Result<ExtensionComponent, maestro_app::EventError> {
        called.borrow_mut().push("new-factory".into());
        Err(Box::new(Original(failure.clone())))
    };
    if side == "header" {
        vec![
            Replacement::Header(api.ui_callback(move |_, _| Ok(old.take().unwrap()))),
            Replacement::Header(api.ui_callback(move |_, _| fail())),
        ]
    } else {
        vec![
            Replacement::Footer(api.ui_callback(move |_, _| Ok(old.take().unwrap()))),
            Replacement::Footer(api.ui_callback(move |_, _| fail())),
        ]
    }
}

/// Installs `factory` through the host.
fn install(
    ui: &maestro_app::ExtensionUIContext,
    factory: Replacement,
) -> Result<(), maestro_app::EventError> {
    match factory {
        Replacement::Header(factory) => ui.set_header(Some(factory)),
        Replacement::Footer(factory) => ui.set_footer(Some(factory)),
    }
}

/// An error whose allocation identity survives host forwarding.
#[derive(Debug)]
struct Original(std::sync::Arc<()>);
impl std::fmt::Display for Original {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("replacement callback failed")
    }
}
impl std::error::Error for Original {}

#[test]
fn replacement_failures_keep_disposal_and_removal_phase() {
    isolated(
        "replacement_failures_keep_disposal_and_removal_phase",
        async |directory| {
            for (side, phase) in [
                ("header", "factory"),
                ("header", "dispose"),
                ("footer", "factory"),
                ("footer", "dispose"),
            ] {
                let identity = std::sync::Arc::new(());
                let (trace, error) = (Trace::default(), identity.clone());
                let recorded = trace.clone();
                let (world, factories) = with_factories(directory, move |api| {
                    failing(api, (side, phase), &recorded, &error)
                })
                .await;
                let ui = port(&world.mode);
                let mut factories = factories.into_iter();
                install(&ui, factories.next().unwrap()).unwrap();
                world.runtime.settle().unwrap();
                let result = install(&ui, factories.next().unwrap());
                let error = result.unwrap_err();
                assert!(std::sync::Arc::ptr_eq(
                    &error.downcast_ref::<Original>().unwrap().0,
                    &identity
                ));
                assert_eq!(world.runtime.pending(), 0);
                let expected: &[&str] = if phase == "dispose" {
                    &["dispose-old"]
                } else {
                    &["dispose-old", "new-factory"]
                };
                assert_eq!(*trace.borrow(), expected);
                let footer_shown = footer_text(&world) == ["OLD"];
                if side == "header" {
                    assert!(header_text(&world).contains(&"OLD".to_owned()));
                } else {
                    assert_eq!(footer_shown, phase == "dispose");
                    let built_in: ComponentHandle = world.mode.footer.clone();
                    assert!(!same(&last_root(&world), &built_in));
                }
            }
        },
    );
}

/// A footer painting the live statuses of the metadata its factory received.
struct StatusFooter(Rc<dyn ReadonlyFooterDataProvider>);
impl Component for StatusFooter {
    fn render(&self, _width: usize) -> Vec<String> {
        let statuses = self.0.get_extension_statuses();
        let mut rows: Vec<String> = statuses.iter().map(|(k, v)| format!("{k}:{v}")).collect();
        rows.push(format!(
            "providers:{}",
            self.0.get_available_provider_count()
        ));
        rows.push(format!(
            "branch:{}",
            self.0.get_git_branch().unwrap_or_default()
        ));
        rows
    }
}

#[test]
fn custom_footer_retains_the_live_status_provider() {
    isolated(
        "custom_footer_retains_the_live_status_provider",
        async |directory| {
            let (world, factories) = with_factories(directory, |api| {
                vec![api.ui_callback(|(_, _, data), _| {
                    Ok(ExtensionComponent {
                        component: Rc::new(StatusFooter(data)),
                        ..plain("")
                    })
                })]
            })
            .await;
            let ui = port(&world.mode);
            let factory: FooterFactory = factories.into_iter().next().unwrap();
            ui.set_status("z".into(), Some("one".into())).unwrap();
            ui.set_footer(Some(factory)).unwrap();
            let count = world
                .mode
                .footer_data_provider
                .get_available_provider_count();
            let painted = |status: &str, providers: f64, branch: &str| {
                vec![
                    status.to_owned(),
                    format!("providers:{providers}"),
                    format!("branch:{branch}"),
                ]
            };
            assert_eq!(footer_text(&world), painted("z:one", count, ""));
            for branch in ["first", "second"] {
                super::with_held_response(&world, async |_| {}).await;
                let repo = directory.join(branch);
                std::fs::create_dir_all(repo.join(".git")).unwrap();
                std::fs::write(
                    repo.join(".git/HEAD"),
                    format!("ref: refs/heads/{branch}\n"),
                )
                .unwrap();
                world
                    .mode
                    .footer_data_provider
                    .set_cwd(repo.to_str().unwrap().to_owned());
                world
                    .mode
                    .footer_data_provider
                    .set_available_provider_count(7.0);
                ui.set_status("z".into(), Some(branch.into())).unwrap();
                assert_eq!(
                    footer_text(&world),
                    painted(&format!("z:{branch}"), 7.0, branch)
                );
            }
        },
    );
}

/// Whether the built-in header shows its expanded help.
fn built_in_expanded(world: &World) -> bool {
    let header = world.mode.built_in_header.borrow().clone().unwrap();
    let text = strip_ansi(&header.render(80).join("\n")).unwrap();
    !text.contains("to show full startup help")
}

#[test]
fn custom_header_expansion_tracks_current_component() {
    isolated(
        "custom_header_expansion_tracks_current_component",
        async |directory| {
            for initially in [false, true] {
                let trace = Trace::default();
                let recorded = trace.clone();
                let (world, factories) = with_factories(directory, move |api| {
                    let plain_header: ComponentFactory = api.ui_callback(|_, _| Ok(plain("plain")));
                    vec![header(api, "HEADER", &recorded), plain_header]
                })
                .await;
                let ui = port(&world.mode);
                ui.set_tools_expanded(initially).unwrap();
                let chat = Rc::new(
                    ExpandableText::new(
                        Box::new(|| Ok("chat:collapsed".into())),
                        Box::new(|| Ok("chat:expanded".into())),
                        initially,
                        0,
                        0,
                    )
                    .unwrap(),
                );
                world.mode.chat_container.add_child(chat.clone());
                let chat_expanded = || chat.render(80)[0].trim_end() == "chat:expanded";
                let mut factories = factories.into_iter();
                ui.set_header(factories.next()).unwrap();
                ui.set_tools_expanded(!initially).unwrap();
                assert_eq!(chat_expanded(), !initially);
                assert_eq!(built_in_expanded(&world), initially);
                assert!(header_text(&world).contains(&"HEADER".to_owned()));
                ui.set_header(None).unwrap();
                assert_eq!(built_in_expanded(&world), !initially);
                ui.set_tools_expanded(initially).unwrap();
                assert_eq!(built_in_expanded(&world), initially);
                assert_eq!(chat_expanded(), initially);
                assert_eq!(
                    *trace.borrow(),
                    [
                        "factory:HEADER".to_owned(),
                        format!("HEADER:expanded:{initially}"),
                        format!("HEADER:expanded:{}", !initially),
                        "HEADER:dispose".to_owned(),
                    ]
                );
                ui.set_header(factories.next()).unwrap();
                ui.set_tools_expanded(!initially).unwrap();
                assert!(header_text(&world).contains(&"plain".to_owned()));
                assert_eq!(built_in_expanded(&world), initially);
            }
        },
    );
}
