use crate::app::{App, Tab};
use crossterm::event::{KeyCode, KeyEvent};

impl App {
    /// Handle a key press. Returns `true` if the app should quit.
    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> bool {
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
                self.active_tab = Tab::Help;
                false
            }

            // Scrolling
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_down();
                false
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll_up();
                false
            }

            // Filter
            KeyCode::Char('/') => {
                if self.active_tab == Tab::Processes {
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

            // Refresh
            KeyCode::Char('r') => {
                self.refresh();
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

            _ => {
                false
            }
        };

        // Reset pending 'g' key if any other key is pressed
        if !matches!(key.code, KeyCode::Char('g')) {
            self.pending_g = false;
        }

        quit
    }
}
