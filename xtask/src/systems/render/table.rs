use std::fmt::Write as _;

use super::text::cell;

pub struct Table {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Table {
    pub fn new(headers: &[&str]) -> Self {
        Self {
            headers: headers.iter().map(|value| value.to_string()).collect(),
            rows: Vec::new(),
        }
    }

    pub fn row<T: IntoIterator<Item = S>, S: Into<String>>(&mut self, cells: T) {
        self.rows.push(
            cells
                .into_iter()
                .map(Into::into)
                .map(|value| cell(&value))
                .collect(),
        );
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "| {} |", self.headers.join(" | "));
        let _ = writeln!(
            out,
            "|{}|",
            self.headers
                .iter()
                .map(|_| "---")
                .collect::<Vec<_>>()
                .join("|")
        );
        for row in &self.rows {
            let _ = writeln!(out, "| {} |", row.join(" | "));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_escapes_cells() {
        let mut table = Table::new(&["A", "B"]);
        table.row(["a|b", "line\nbreak"]);
        assert_eq!(
            table.render(),
            "| A | B |\n|---|---|\n| a\\|b | line break |\n"
        );
    }
}
