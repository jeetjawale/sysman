use crate::app::{App, Tab};
use crossterm::event::{KeyCode, KeyEvent};

impl App {
    /// Handle a key press. Returns `true` if the app should quit.
    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> bool {
        if self.logs_regex_input {
            match key.code {
                KeyCode::Esc => {
                    self.logs_regex_input = false;
                    self.logs_query.clear();
                }
                KeyCode::Enter => {
                    self.logs_regex_input = false;
                }
                KeyCode::Backspace => {
                    self.logs_query.pop();
                }
                KeyCode::Char(ch) => self.logs_query.push(ch),
                _ => {}
            }
            return false;
        }

        if self.network_tool_input {
            match key.code {
                KeyCode::Esc => {
                    self.network_tool_input = false;
                    self.network_tool_value.clear();
                }
                KeyCode::Enter => {
                    self.network_tool_input = false;
                    self.run_network_tools();
                    self.network_tool_value.clear();
                }
                KeyCode::Backspace => {
                    self.network_tool_value.pop();
                }
                KeyCode::Char(ch) => self.network_tool_value.push(ch),
                _ => {}
            }
            return false;
        }

        if self.pin_input {
            match key.code {
                KeyCode::Esc => {
                    self.pin_input = false;
                    self.pin_core_value.clear();
                }
                KeyCode::Enter => {
                    self.pin_input = false;
                    self.apply_pin_selected();
                    self.pin_core_value.clear();
                }
                KeyCode::Backspace => {
                    self.pin_core_value.pop();
                }
                KeyCode::Char(ch) if ch.is_ascii_digit() => {
                    self.pin_core_value.push(ch);
                }
                _ => {}
            }
            return false;
        }

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
                self.active_tab = Tab::Overview;
                false
            }
            KeyCode::Char('2') => {
                self.active_tab = Tab::Cpu;
                false
            }
            KeyCode::Char('3') => {
                self.active_tab = Tab::Memory;
                false
            }
            KeyCode::Char('4') => {
                self.active_tab = Tab::Processes;
                self.refresh_selected_process_details();
                false
            }
            KeyCode::Char('5') => {
                self.active_tab = Tab::Network;
                false
            }
            KeyCode::Char('6') => {
                self.active_tab = Tab::Disk;
                false
            }
            KeyCode::Char('7') => {
                self.active_tab = Tab::Gpu;
                false
            }
            KeyCode::Char('8') => {
                self.active_tab = Tab::Services;
                self.refresh_selected_service_logs();
                self.refresh_selected_service_failure_details();
                false
            }
            KeyCode::Char('9') => {
                self.active_tab = Tab::Logs;
                self.refresh_logs_view();
                false
            }
            KeyCode::Char('0') => {
                self.active_tab = Tab::Hardware;
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
                    self.pin_input = false;
                    self.renice_input = false;
                    self.filter_input = true;
                } else if self.active_tab == Tab::Logs {
                    self.filter_input = false;
                    self.renice_input = false;
                    self.pin_input = false;
                    self.network_tool_input = false;
                    self.logs_regex_input = true;
                }
                false
            }
            KeyCode::Esc => {
                if self.active_tab == Tab::Processes {
                    self.process_filter.clear();
                } else if self.active_tab == Tab::Logs {
                    self.logs_query.clear();
                }
                false
            }

            // Sort
            KeyCode::Char('s') => {
                if self.active_tab == Tab::Processes {
                    self.cycle_process_sort();
                    self.refresh();
                } else if self.active_tab == Tab::Services {
                    self.cycle_service_state_filter();
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
                } else if self.active_tab == Tab::Network {
                    self.kill_selected_connection();
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
                    self.pin_input = false;
                    self.renice_input = true;
                    self.renice_value.clear();
                } else if self.active_tab == Tab::Logs {
                    self.navigate_logs_match(true);
                }
                false
            }
            KeyCode::Char('N') => {
                if self.active_tab == Tab::Logs {
                    self.navigate_logs_match(false);
                }
                false
            }
            KeyCode::Char('a') => {
                if self.active_tab == Tab::Processes {
                    self.filter_input = false;
                    self.renice_input = false;
                    self.pin_input = true;
                    self.pin_core_value.clear();
                } else if self.active_tab == Tab::Logs {
                    self.toggle_logs_autoscroll();
                }
                false
            }
            KeyCode::Char('f') => {
                if self.active_tab == Tab::Disk {
                    self.scan_selected_disk_dirs();
                }
                false
            }
            KeyCode::Char('m') => {
                if self.active_tab == Tab::Disk {
                    self.cycle_disk_scan_depth();
                }
                false
            }
            KeyCode::Char('v') => {
                if self.active_tab == Tab::Logs {
                    self.cycle_logs_level_filter();
                }
                false
            }
            KeyCode::Char('o') => {
                if self.active_tab == Tab::Services {
                    self.act_on_selected_service("restart");
                } else if self.active_tab == Tab::Logs {
                    self.cycle_logs_source_filter();
                }
                false
            }
            KeyCode::Char('b') => {
                if self.active_tab == Tab::Network {
                    self.block_selected_remote_ip();
                }
                false
            }
            KeyCode::Char('c') => {
                if self.active_tab == Tab::Network {
                    self.cycle_connection_state_filter();
                }
                false
            }
            KeyCode::Char('t') => {
                if self.active_tab == Tab::Network {
                    self.filter_input = false;
                    self.renice_input = false;
                    self.pin_input = false;
                    self.network_tool_input = true;
                    self.network_tool_value.clear();
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
            KeyCode::Char('w') => {
                if self.active_tab == Tab::Services {
                    self.act_on_selected_service("mask");
                }
                false
            }
            KeyCode::Char('W') => {
                if self.active_tab == Tab::Services {
                    self.act_on_selected_service("unmask");
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
