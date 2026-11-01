use super::*;
use egui::{Color32, RichText};
use theme::*;

impl Workspace {
    pub(super) fn navigation(&mut self, ctx: &egui::Context) {
        let dark = Color32::from_rgb(25, 32, 29);
        let quiet = Color32::from_rgb(153, 166, 155);
        egui::SidePanel::left("navigation")
            .exact_width(194.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(dark).inner_margin(16))
            .show(ctx, |ui| {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(30.0, 30.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 8, ACCENT);
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "t",
                        egui::FontId::proportional(25.0),
                        Color32::WHITE,
                    );
                    ui.label(
                        RichText::new("tesserun")
                            .size(23.0)
                            .strong()
                            .color(Color32::WHITE),
                    );
                });
                ui.add_space(25.0);
                if ui
                    .add_sized(
                        [162.0, 34.0],
                        egui::Button::new(RichText::new("+   New project").color(Color32::WHITE))
                            .fill(Color32::from_rgb(48, 61, 51)),
                    )
                    .clicked()
                {
                    if self.dirty {
                        self.status =
                            "Save or discard your edits before starting a new project.".into();
                    } else if !self.tasks.is_busy() {
                        self.selected = None;
                        self.prompt.clear();
                        self.report = None;
                        self.deployment = None;
                        self.export_path = None;
                        self.tab = 0;
                    }
                }
                ui.add_space(22.0);
                for (index, label) in [
                    (0, "Development"),
                    (3, "Quality checks"),
                    (4, "Deployments"),
                    (1, "AI chat"),
                    (2, "Settings"),
                ] {
                    let selected = self.tab == index;
                    if ui
                        .add_sized(
                            [162.0, 34.0],
                            egui::Button::new(RichText::new(label).color(if selected {
                                Color32::WHITE
                            } else {
                                quiet
                            }))
                            .fill(if selected {
                                Color32::from_rgb(48, 61, 51)
                            } else {
                                Color32::TRANSPARENT
                            })
                            .stroke(egui::Stroke::NONE),
                        )
                        .clicked()
                    {
                        self.tab = index;
                    }
                }
                ui.add_space(26.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("RECENT PROJECTS").size(10.0).color(quiet));
                    if ui
                        .add_enabled(
                            !self.tasks.is_busy(),
                            egui::Button::new(RichText::new("↻").color(quiet)).frame(false),
                        )
                        .on_hover_text("Refresh projects")
                        .clicked()
                    {
                        self.refresh(ctx.clone());
                    }
                });
                egui::ScrollArea::vertical()
                    .max_height((ui.available_height() - 95.0).max(80.0))
                    .show(ui, |ui| {
                        if self.projects.is_empty() {
                            ui.label(
                                RichText::new("Your projects will appear here.")
                                    .size(11.0)
                                    .color(quiet),
                            );
                        }
                        for project in &self.projects {
                            if ui
                                .add_enabled(
                                    !self.tasks.is_busy(),
                                    egui::Button::new(
                                        RichText::new(
                                            project.prompt.chars().take(22).collect::<String>(),
                                        )
                                        .color(quiet),
                                    )
                                    .frame(false),
                                )
                                .clicked()
                            {
                                if self.dirty {
                                    self.status =
                                        "Save or discard your edits before switching projects."
                                            .into();
                                } else {
                                    self.selected = Some(project.clone());
                                    self.file = 0;
                                    self.tab = 0;
                                    self.report = None;
                                    self.deployment = None;
                                    self.export_path = None;
                                    self.export_confirmed = false;
                                    self.deploy_confirmed = false;
                                }
                            }
                        }
                    });
                ui.add_space(25.0);
                ui.label(
                    RichText::new("Built on your machine.")
                        .size(11.0)
                        .color(quiet),
                );
                ui.label(
                    RichText::new("Powered by your models.")
                        .size(11.0)
                        .color(quiet),
                );
            });
    }
    pub(super) fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("workspace-header")
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::symmetric(24, 14)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.strong(match self.tab {
                        1 => "AI conversation",
                        2 => "Connections",
                        3 => "Testing",
                        4 => "Deployment",
                        _ => "Development",
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(
                                RichText::new(if self.connection.model.is_empty() {
                                    "Choose a model".into()
                                } else {
                                    format!(
                                        "{} / {}",
                                        self.connection.provider, self.connection.model
                                    )
                                })
                                .small()
                                .color(ACCENT),
                            )
                            .clicked()
                        {
                            self.tab = 2;
                        }
                        if ui
                            .selectable_label(self.show_agents, "Agent activity")
                            .clicked()
                        {
                            self.show_agents = !self.show_agents;
                        }
                        if self.dirty {
                            ui.label(RichText::new("Unsaved changes").small().color(AMBER));
                        }
                    });
                });
            });
    }
    pub(super) fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status")
            .frame(egui::Frame::new().fill(PANEL).inner_margin(12))
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if self.tasks.is_busy() {
                        ui.spinner();
                    }
                    ui.label(RichText::new(&self.status).small().color(MUTED));
                    if self.tasks.is_busy() && ui.small_button("Cancel operation").clicked() {
                        self.tasks.cancel();
                        self.status =
                            "Operation cancelled. Completed source revisions remain saved.".into();
                    }
                });
            });
    }
    pub(super) fn settings_view(&mut self, ui: &mut egui::Ui) {
        title(
            ui,
            "CONNECTIONS",
            "Bring your own intelligence",
            "Choose the provider used by your development agents and conversations.",
        );
        card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.strong("Provider configuration");
            ui.add_space(8.0);
            egui::ComboBox::from_label("Provider")
                .selected_text(&self.connection.provider)
                .show_ui(ui, |ui| {
                    for provider in ["ollama", "openai", "anthropic", "gemini", "groq"] {
                        ui.selectable_value(
                            &mut self.connection.provider,
                            provider.into(),
                            provider,
                        );
                    }
                });
            ui.label("Model ID");
            ui.add(
                egui::TextEdit::singleline(&mut self.connection.model)
                    .hint_text("Enter an available model ID")
                    .desired_width(440.0),
            );
            if self.connection.provider == "ollama" {
                ui.label("Local Ollama origin");
                ui.text_edit_singleline(&mut self.connection.endpoint);
            } else {
                ui.label("API key");
                ui.add(
                    egui::TextEdit::singleline(&mut self.connection.api_key)
                        .password(true)
                        .hint_text("Session only, or configure your provider environment variable")
                        .desired_width(440.0),
                );
            }
            ui.add_space(12.0);
            ui.label(
                RichText::new(
                    "Keys stay in memory. Configuration does not establish provider connectivity.",
                )
                .small()
                .color(MUTED),
            );
        });
    }
    pub(super) fn chat_view(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        title(
            ui,
            "AI CONVERSATION",
            "Think through the next move",
            "Explore an idea with your configured provider. Conversation is session-only.",
        );
        card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            egui::ScrollArea::vertical().max_height((ui.available_height()-160.0).max(100.0)).stick_to_bottom(true).show(ui, |ui| {
                if self.messages.is_empty() { ui.add_space(35.0); ui.label(RichText::new("What would you like to build?").size(24.0).color(ACCENT)); ui.label("Discuss architecture, clarify a requirement, or work through a problem."); ui.add_space(35.0); }
                for message in &self.messages { ui.label(RichText::new(if message.role == "user" {"YOU"} else {"ASSISTANT"}).small().strong().color(ACCENT)); ui.label(&message.content); ui.add_space(12.0); }
            });
            ui.add(egui::TextEdit::multiline(&mut self.draft).desired_rows(3).desired_width(f32::INFINITY).hint_text("Ask a question…"));
            if ui.add_enabled(!self.tasks.is_busy() && !self.draft.trim().is_empty(), primary("Send message")).clicked() {
                self.messages.push(Message { role:"user".into(),content:std::mem::take(&mut self.draft) });
                let messages=self.messages.clone(); let service=self.service.clone(); let connection=self.connection.clone();
                self.launch(ctx.clone(),async move { Ok(Event::Reply(service.providers.complete(&connection,&messages).await?)) });
            }
        });
    }
    pub(super) fn build_view(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(if self.selected.is_none() { 36.0 } else { 10.0 });
            ui.label(RichText::new("A LITTLE IDEA. A BIG POSSIBILITY.").size(10.0).strong().color(ACCENT));
            ui.add_space(10.0);
            ui.label(RichText::new(if self.selected.is_none() { "What will you create?" } else { "Make your next iteration count." }).size(42.0).family(egui::FontFamily::Name("display".into())));
            ui.add_space(4.0);
            ui.label(RichText::new("Turn a clear brief into working source, with a team that thinks it through.").size(15.0).color(MUTED));
            ui.add_space(26.0);
            card().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.add(egui::TextEdit::multiline(&mut self.prompt).frame(false).desired_rows(5).desired_width(f32::INFINITY).font(egui::FontId::proportional(18.0)).hint_text(RichText::new("Describe the app you have in mind…\nWho is it for? What should it help them do?").color(MUTED)));
                ui.add_space(16.0);ui.separator();ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    if ui.add(egui::Button::new(RichText::new(if self.connection.model.is_empty() {"○   Select a model".into()} else {format!("{}  /  {}",self.connection.provider,self.connection.model)}).size(12.0).color(MUTED)).frame(false)).clicked() {self.tab=2;}
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let missing_model=self.connection.model.is_empty();
                        let ready=!self.tasks.is_busy() && !self.dirty && (missing_model || !self.prompt.trim().is_empty());
                        if ui.add_enabled(ready,primary(if missing_model {"Connect a model"} else {"Start building"})).clicked() {
                            if missing_model {self.tab=2;}
                            else {
                                self.report=None;self.deployment=None;self.export_confirmed=false;self.deploy_confirmed=false;
                                let service=self.service.clone();let connection=self.connection.clone();let prompt=self.prompt.clone();
                                self.launch(ctx.clone(),async move {Ok(Event::Developed(crate::workflow::develop(&service,prompt,connection).await?))});
                            }
                        }
                    });
                });
            });
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| { ui.label(RichText::new("TRY AN IDEA").size(10.0).color(MUTED));
                for (label,brief) in [("Personal dashboard","Build a personal dashboard to track tasks, goals, and daily focus."),("Product landing page","Build a clean product landing page with features, pricing, and a contact form."),("Habit tracker","Build a habit tracker with daily check-ins, streaks, and a weekly overview.")] {
                    if ui.add(egui::Button::new(RichText::new(label).size(11.0)).corner_radius(egui::CornerRadius::same(14))).clicked() {self.prompt=brief.into();}
                }
            });
            ui.add_space(30.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("ONE BRIEF, A SPECIALIST TEAM").size(10.0).color(MUTED));
                if ui.link(if self.show_agents {"Hide team"} else {"Meet the team"}).clicked() {self.show_agents = !self.show_agents;}
            });
            ui.add_space(10.0);
            ui.columns(3, |columns| {
                for (column,(label,detail)) in columns.iter_mut().zip([("01   Think it through","A planner turns your brief into a clear approach."),("02   Build & review","A developer writes source. A reviewer challenges it."),("03   Check & ship","Inspect source checks, export, or publish when ready.")]) {
                    column.label(RichText::new(label).size(14.0).strong());column.label(RichText::new(detail).size(12.0).color(MUTED));
                }
            });
            ui.add_space(12.0);
            ui.label(RichText::new("Three model requests per build. Source checks do not run the generated application.").size(10.0).color(MUTED));
            ui.add_space(28.0);
            let mut save=None;
            if let Some(project)=&mut self.selected {
                card().show(ui, |ui| {
                    ui.horizontal(|ui| { ui.strong("Source workspace"); ui.label(RichText::new(format!("{} files",project.files.len())).small().color(MUTED)); });
                    ui.horizontal_wrapped(|ui| { for (index,file) in project.files.iter().enumerate() { ui.selectable_value(&mut self.file,index,&file.path); } });
                    if let Some(file)=project.files.get_mut(self.file)
                        && ui.add_enabled(!self.tasks.is_busy(),egui::TextEdit::multiline(&mut file.content).code_editor().desired_rows(18).desired_width(f32::INFINITY)).changed() {
                            self.dirty=true; self.report=None; self.deployment=None; self.export_path=None; self.deploy_confirmed=false; self.export_confirmed=false;
                    }
                    ui.horizontal_wrapped(|ui| {
                        if ui.add_enabled(!self.tasks.is_busy() && self.dirty,egui::Button::new("Save revision")).clicked() { save=Some(project.clone()); }
                        if ui.add_enabled(!self.tasks.is_busy() && self.dirty,egui::Button::new("Discard edits")).clicked()
                            && let Some(saved)=self.projects.iter().find(|p|p.id==project.id) { *project=saved.clone(); self.dirty=false; }
                        if ui.button("Open testing").clicked() { self.tab=3; }
                    });
                });
            }
            if let Some(snapshot)=save { let store=self.service.store.clone(); self.launch(ctx.clone(),async move { store.save(&snapshot).await?; Ok(Event::Saved(snapshot)) }); }
        });
    }
}
