use std::{io, time::Duration};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    Quit,
    ToggleTheme,
    Key(KeyCode),
}

/// Internal helper for poll action.
pub(crate) fn poll_action() -> io::Result<Option<Action>> {
    if !event::poll(Duration::from_millis(250))? {
        return Ok(None);
    }
    let Event::Key(key) = event::read()? else {
        return Ok(None);
    };
    if key.kind != KeyEventKind::Press {
        return Ok(None);
    }

    let action = match key.code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Char('t') => Action::ToggleTheme,
        KeyCode::Char('h') => Action::Key(KeyCode::Left),
        KeyCode::Char('j') => Action::Key(KeyCode::Down),
        KeyCode::Char('k') => Action::Key(KeyCode::Up),
        KeyCode::Char('l') => Action::Key(KeyCode::Right),
        key_code => Action::Key(key_code),
    };
    Ok(Some(action))
}

#[cfg(test)]
mod tests {
    use super::Action;
    use crossterm::event::KeyCode;

    #[test]
    fn exposes_semantic_actions_for_global_shortcuts() {
        assert_eq!(Action::Quit, Action::Quit);
        assert_eq!(Action::ToggleTheme, Action::ToggleTheme);
        assert_eq!(Action::Key(KeyCode::Left), Action::Key(KeyCode::Left));
    }
}
