//! These tools query the already-redacted snapshot. Their arguments never become OS paths.
use super::{isolated_source::SourceFile, Excerpt};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Query {
    Search { query: String },
    Read { path: String, line: usize },
}

pub(super) const ROUNDS: usize = 2;

fn lines_at(
    file: &SourceFile,
    start: usize,
    count: usize,
    max_bytes: usize,
) -> Option<Excerpt<'_>> {
    if start == 0 {
        return None;
    }
    let text = file.text.as_str();
    let offset = if start == 1 {
        0
    } else {
        text.match_indices('\n').nth(start - 2)?.0 + 1
    };
    let tail = &text[offset..];
    let end = tail
        .match_indices('\n')
        .nth(count - 1)
        .map_or(tail.len(), |(i, _)| i + 1);
    let mut length = end.min(max_bytes);
    while !tail.is_char_boundary(length) {
        length -= 1;
    }
    (length > 0).then(|| Excerpt {
        path: &file.path,
        first_line: start,
        text: &tail[..length],
    })
}

pub(super) fn resolve<'a>(
    queries: &[Query],
    files: &'a [SourceFile],
) -> Result<(Vec<Excerpt<'a>>, String), String> {
    if !(1..=2).contains(&queries.len()) {
        return Err("A consulta deve conter uma ou duas operações.".into());
    }
    let mut results = Vec::new();
    let mut searches = 0;
    let mut reads = 0;
    for query in queries {
        match query {
            Query::Read { path, line } => {
                if path.is_empty() || path.len() > 4096 || path.contains('\0') || *line == 0 {
                    return Err("Pedido de leitura inválido.".into());
                }
                reads += 1;
                if let Some(snippet) = files
                    .iter()
                    .find(|f| f.path == *path)
                    .and_then(|f| lines_at(f, *line, 24, 3072))
                {
                    results.push(snippet);
                }
            }
            Query::Search { query } => {
                if !(2..=160).contains(&query.len())
                    || query.chars().any(char::is_control)
                    || query.trim().is_empty()
                {
                    return Err("Pedido de busca inválido.".into());
                }
                searches += 1;
                let mut hits = 0;
                for file in files {
                    for (number, line) in file.text.lines().enumerate() {
                        if line.contains(query) {
                            if let Some(snippet) = lines_at(file, number + 1, 3, 512) {
                                results.push(snippet);
                            }
                            hits += 1;
                            if hits == 6 {
                                break;
                            }
                        }
                    }
                    if hits == 6 {
                        break;
                    }
                }
            }
        }
    }
    let activity = format!("Consulta à cópia autorizada: {searches} busca(s), {reads} leitura(s), {} trecho(s) retornado(s).", results.len());
    Ok((results, activity))
}
