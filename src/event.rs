use crate::app::{App, Tab};
use crossterm::event::{KeyCode, KeyEvent};

impl App {
    /// Handle a key press. Returns `true` if the app should quit.
    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> bool {
        if self.renice_input {
            match key.code {
                KeyCode::Esc => {
                    self.renice_input = false;
                    self.renice_value.clear();
                }
                KeyCode::Enter => {
                    self.renice_input = false;
                    self.apply_renice_selected();
                    self.renice_value.clear();
                }
                KeyCode::Backspace => {
                    self.renice_value.pop();
                }
                KeyCode::Char(ch) if ch.is_ascii_digit() || ch == '-' => {
                    if ch == '-' && !self.renice_value.is_empty() {
                        return false;
                    }
                    if ch != '-' || !self.renice_value.starts_with('-') {
                        self.renice_value.push(ch);
                    }
                }
                _ => {}
            }
            return false;
        }

        // Filter input mode: capture text for the process filter
        if self.filter_input {
            match key.code {
                KeyCode::Esc => self.filter_input = false,
                KeyCode::Enter => self.filter_input = false,
                KeyCode::Backspace => {
                    self.process_filter.pop();
                }
                KeyCode::Char(ch) => self.process_filter.push(ch),
                _ => {}
            }
            return false;
        }

        let quit = match key.code {
            KeyCode::Char('q') => true,

            // Tab navigation
            KeyCode::Left | KeyCode::Char('h') => {
                self.previous_tab();
                false
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.next_tab();
                false
            }
            KeyCode::Char('1') => {
                self.active_tab = Tab::Dashboard;
                false
            }
            KeyCode::Char('2') => {
                self.active_tab = Tab::System;
                false
            }
            KeyCode::Char('3') => {
                self.active_tab = Tab::Processes;
                false
            }
            KeyCode::Char('4') => {
                self.active_tab = Tab::Network;
                false
            }
            KeyCode::Char('5') => {
                self.active_tab = Tab::Disks;
                false
            }
            KeyCode::Char('6') => {
                self.active_tab = Tab::Services;
                false
            }
            KeyCode::Char('7') => {
                self.active_tab = Tab::Logs;
                self.refresh_logs_view();
                false
            }
            KeyCode::Char('8') => {
                self.active_tab = Tab::Help;
                false
            }
            KeyCode::Char('?') => {
                self.active_tab = Tab::Help;
                false
            }

            // Scrolling
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_down();
                false
            }
            KeyCode::Up => {
                self.scroll_up();
                false
            }
            KeyCode::Char('k') => {
                self.scroll_up();
                false
            }

            // Filter
            KeyCode::Char('/') => {
                if self.active_tab == Tab::Processes {
                    self.renice_input = false;
                    self.filter_input = true;
                }
                false
            }
            KeyCode::Esc => {
                self.process_filter.clear();
                false
            }

            // Sort
            KeyCode::Char('s') => {
                if self.active_tab == Tab::Processes {
                    self.cycle_process_sort();
                    self.refresh();
                }
                false
            }
            KeyCode::Char('p') => {
                if self.active_tab == Tab::Processes {
                    self.cycle_process_view();
                }
                false
            }

            // Process actions
            KeyCode::Char('x') => {
                if self.active_tab == Tab::Processes {
                    self.kill_selected_process(false);
                }
                false
            }
            KeyCode::Char('z') => {
                if self.active_tab == Tab::Processes {
                    self.kill_selected_process(true);
                }
                false
            }
            KeyCode::Char('r') => {
                self.refresh();
                false
            }
            KeyCode::Char('n') => {
                if self.active_tab == Tab::Processes {
                    self.filter_input = false;
                    self.renice_input = true;
                    self.renice_value.clear();
                }
                false
            }
            KeyCode::Char('f') => {
                if self.active_tab == Tab::Disks {
                    self.scan_selected_disk_dirs();
                }
                false
            }

            // Service actions
            KeyCode::Char('u') => {
                if self.active_tab == Tab::Services {
                    self.act_on_selected_service("start");
                }
                false
            }
            KeyCode::Char('i') => {
                if self.active_tab == Tab::Services {
                    self.act_on_selected_service("stop");
                }
                false
            }
            KeyCode::Char('o') => {
                if self.active_tab == Tab::Services {
                    self.act_on_selected_service("restart");
                }
                false
            }
            KeyCode::Char('e') => {
                if self.active_tab == Tab::Services {
                    self.act_on_selected_service("enable");
                }
                false
            }
            KeyCode::Char('d') => {
                if self.active_tab == Tab::Services {
                    self.act_on_selected_service("disable");
                }
                false
            }

            // Vim-style jump
            KeyCode::Char('g') => {
                if self.pending_g {
                    self.scroll_top();
                    self.pending_g = false;
                } else {
                    self.pending_g = true;
                }
                false
            }
            KeyCode::Char('G') => {
                self.scroll_bottom();
                self.pending_g = false;
                false
            }

            _ => false,
        };

        // Reset pending 'g' key if any other key is pressed
        if !matches!(key.code, KeyCode::Char('g')) {
            self.pending_g = false;
        }

        quit
    }
}
