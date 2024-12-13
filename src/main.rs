use std::collections::HashMap;
use std::fs::read_to_string;
use std::io;
use std::str::FromStr;

fn main() -> io::Result<()> {
    let extension = "csv";
    let folder = "data";
    let files = ["1", "2", "3", "4", "budget"];

    let paths: Vec<String> = files
        .into_iter()
        .map(|file| format!("{folder}/{file}.{extension}"))
        .collect();

    for contents in paths
        .into_iter()
        .filter_map(|path| read_to_string(path).ok())
    {
        let (csv, size) = parse_csv(&contents);
        display_table(csv, size);

        println!("{}", "-".repeat(20));
    }

    Ok(())
}

fn display_table(table: HashMap<Pos, Status<Cell>>, size: Pos) {
    let mut rows = Vec::new();
    for y in 0..size.y {
        let mut comments = Vec::new();
        let mut values = Vec::new();
        for x in 0..size.x {
            let pos = Pos::new(x as usize, y as usize);
            if let Some(cell) = table.get(&pos) {
                let (value, comment) = match cell {
                    Status::Error => (Some("Error".to_string()), None),
                    Status::Empty => (Some("".to_string()), None),
                    Status::Pending(cell) => (
                        cell.value.as_ref().map(|val| format!("PENDING: {val}")),
                        cell.comment,
                    ),
                    Status::Finished(cell) => (
                        cell.value.as_ref().map(|val| format!("{val}")),
                        cell.comment,
                    ),
                };
                comments.push(comment);
                values.push(value);
            } else {
                comments.push(None);
                values.push(None);
            }
        }
        rows.push((comments, values));
    }

    let mut max_lengths = Vec::new();
    for (_y, row) in rows.iter().enumerate() {
        let (comments, values) = row;
        for (i, (comment, value)) in comments.into_iter().zip(values.into_iter()).enumerate() {
            while i >= max_lengths.len() {
                max_lengths.push(0);
            }
            let max = comment
                .unwrap_or("")
                .len()
                .max(value.clone().unwrap_or("".to_string()).len());
            if max > max_lengths[i] {
                max_lengths[i] = max;
            }
        }
    }
    for (_y, (comments, values)) in rows.iter().enumerate() {
        let mut comments_line = String::new();
        let mut values_line = String::new();
        for (i, (comment, value)) in comments.iter().zip(values.iter()).enumerate() {
            let space = max_lengths[i];
            let comment = comment.unwrap_or("");
            let padding = " ".repeat(space - comment.len());
            let formatted_comment = format!("{comment}{padding}");
            let value = value.clone().unwrap_or("".to_string());
            let padding = " ".repeat(space - value.len());
            let formatted_value = format!("{value}{padding}");
            comments_line = format!("{comments_line}{formatted_comment} ");
            values_line = format!("{values_line}{formatted_value} ");
        }
        println!("{}", comments_line);
        println!("{}", values_line);
    }
}

fn parse_csv(contents: &str) -> (HashMap<Pos, Status<Cell>>, Pos) {
    let lines = contents.lines().enumerate();
    let mut cells = HashMap::new();
    let (mut max_x, mut max_y) = (0, 0);
    for (y, line) in lines {
        for (x, cell) in line.split(',').enumerate() {
            cells.insert(Pos::new(x, y), parse_cell(cell));
            if x > max_x {
                max_x = x
            }
        }
        if y > max_y {
            max_y = y
        }
    }
    (cells, Pos::new(max_x + 1, max_y + 1))
}

fn parse_cell(cell_contents: &str) -> Status<Cell> {
    let cell = cell_contents.trim();
    if cell.is_empty() {
        return Status::Empty;
    }
    // Split the cell into content and comment
    let (content, comment) = if let Some((content, comment)) = cell.split_once('#') {
        (content.trim(), Some(comment.trim()))
    } else {
        (cell, None)
    };

    // Attempt to evaluate the content
    let value: Option<Value> = if content.is_empty() {
        None
    } else if let Ok(num) = f64::from_str(content) {
        Some(Value::Number(num))
    } else if let Ok(result) = eval_expression(content) {
        Some(result)
    } else {
        return Status::Pending(Cell {
            original: cell_contents,
            value: Some(Value::String(content.to_string())),
            comment,
        });
    };

    // Return the parsed cell
    let cell = Cell {
        original: cell_contents,
        value,
        comment,
    };
    Status::Finished(cell)
}

// Simple math expression evaluator using the `meval` crate (or similar)
fn eval_expression(expr: &str) -> Result<Value, meval::Error> {
    meval::eval_str(expr).map(|res| Value::Number(res)) // Parses and evaluates the mathematical expression
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash)]
struct Pos {
    x: i32,
    y: i32,
}

impl Pos {
    fn new(x: usize, y: usize) -> Self {
        assert!(x < 20000);
        assert!(y < 20000);
        Self {
            x: x.try_into().unwrap(),
            y: y.try_into().unwrap(),
        }
    }
}

#[derive(Debug, Default)]
enum Status<T: Default> {
    #[default]
    Error,
    Empty,
    Pending(T),
    Finished(T),
}

#[derive(Debug, Default)]
struct Cell<'a> {
    original: &'a str, // Original cell content
    value: Option<Value>,
    comment: Option<&'a str>, // Comment, if present
}

#[derive(Debug)]
enum Value {
    Number(f64),
    String(String),
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Number(num) => f.write_str(&format!("{num:0.03}")),
            Value::String(string) => f.write_str(string),
        }
    }
}
