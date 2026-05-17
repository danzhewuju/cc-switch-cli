use super::*;

pub(super) fn render_hermes_memory(
    frame: &mut Frame<'_>,
    app: &App,
    data: &UiData,
    area: Rect,
    theme: &super::theme::Theme,
) {
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(pane_border_style(app, Focus::Content, theme))
        .title(crate::t!("Memory", "记忆管理"));
    frame.render_widget(outer.clone(), area);
    let inner = outer.inner(area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    if app.focus == Focus::Content {
        render_key_bar_center(
            frame,
            chunks[0],
            theme,
            &[
                ("Enter/e", texts::tui_key_edit()),
                ("Space", crate::t!("Toggle", "切换")),
            ],
        );
    }

    let memory = &data.config.hermes_memory;
    let rows = vec![
        hermes_memory_row(
            "Agent Memory (MEMORY.md)",
            memory.memory_enabled,
            memory.memory_content.len(),
            memory.memory_limit,
            &memory.memory_content,
        ),
        hermes_memory_row(
            "User Profile (USER.md)",
            memory.user_enabled,
            memory.user_content.len(),
            memory.user_limit,
            &memory.user_content,
        ),
    ];

    let table = Table::new(
        rows,
        [
            Constraint::Length(28),
            Constraint::Length(10),
            Constraint::Length(14),
            Constraint::Min(16),
        ],
    )
    .block(Block::default().borders(Borders::NONE))
    .header(Row::new(vec![
        Cell::from(crate::t!("File", "文件")),
        Cell::from(crate::t!("Status", "状态")),
        Cell::from(crate::t!("Usage", "用量")),
        Cell::from(crate::t!("Preview", "预览")),
    ]))
    .row_highlight_style(selection_style(theme))
    .highlight_symbol(highlight_symbol(theme));

    let mut state = TableState::default();
    state.select(Some(app.hermes_memory_idx.min(1)));
    frame.render_stateful_widget(table, inset_left(chunks[1], CONTENT_INSET_LEFT), &mut state);
}

fn hermes_memory_row(
    label: &'static str,
    enabled: bool,
    current: usize,
    limit: usize,
    content: &str,
) -> Row<'static> {
    let status = if enabled {
        crate::t!("enabled", "enabled")
    } else {
        crate::t!("disabled", "disabled")
    };
    Row::new(vec![
        Cell::from(label),
        Cell::from(status),
        Cell::from(format!("{current} / {limit}")),
        Cell::from(memory_preview(content)),
    ])
}

fn memory_preview(content: &str) -> String {
    let preview = content
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    if preview.is_empty() {
        crate::t!("empty", "empty").to_string()
    } else {
        preview.chars().take(80).collect()
    }
}
