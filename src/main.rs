use std::collections::HashMap;
use std::fs::read_to_string;
use std::str::FromStr;
use std::{cell, io};

fn main() -> io::Result<()> {
    let extension = "csv";
    let folder = "data";
    let files = ["1", "2", "3", "4", "budget"];

    let paths: Vec<String> = files
        .into_iter()
        .map(|file| format!("{folder}/{file}.{extension}"))
        .collect();

    let separators = vec![',', ',', ',', '\t', '\t'];
    for (contents, separator) in paths
        .into_iter()
        .filter_map(|path| read_to_string(path).ok())
        .zip(separators.into_iter())
    {
        let (mut csv, size) = parse_csv(&contents, separator);
        for _i in 0..10 {
            // TODO: split off the one value without csv copy
            let csv_copy: HashMap<Pos, Status> = csv.clone();
            for (pos, status) in csv
                .iter_mut()
                .into_iter()
                .filter(|(_k, v)| matches![v, Status::Pending(_)])
            {
                resolve_cell(&csv_copy, status);
            }
        }
        display_table(csv, size);

        println!("{}", "-".repeat(20));
        eprintln!("{}", "-".repeat(20));
    }

    Ok(())
}

fn display_table(table: HashMap<Pos, Status>, size: Pos) {
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

fn parse_csv(contents: &str, separator: char) -> (HashMap<Pos, Status>, Pos) {
    let lines = contents.lines().enumerate();
    let mut cells = HashMap::new();
    let (mut max_x, mut max_y) = (0, 0);
    for (y, line) in lines {
        for (x, cell) in line.split(separator).enumerate() {
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
//fn resolve_cell(context: &HashMap<Pos, Status<Cell>>, pending_cell: &mut Status<Cell>)
fn resolve_cell<'context, 'pending>(
    context: &HashMap<Pos, Status<'context>>,
    pending_cell: &mut Status<'pending>,
) where
    'context: 'pending,
{
    let mut updated_cell = Status::Error;
    if let Status::Pending(cell) = pending_cell {
        match &cell.value.clone().unwrap() {
            Value::String(content) => {
                if content.starts_with('[') {
                    if let Some(index) = content.find(']') {
                        let pos = Pos::try_parse(&content[0..=index]);

                        match pos {
                            // There might be more stuff after position
                            Ok(pos) => {
                                updated_cell = Status::Pending(Cell {
                                    original: cell.original,
                                    value: Some(Value::Pos(pos)),
                                    comment: cell.comment,
                                })
                            }
                            Err(_err) => println!("Error parsing position from {content}!"),
                        }
                    }
                } else if content.starts_with("Span(") {
                    if let Some(index) = content.find(')') {
                        let span = try_parse_span(&content[0..=index]);

                        match span {
                            // There might be more stuff after position
                            Ok(span) => {
                                updated_cell = Status::Pending(Cell {
                                    original: cell.original,
                                    value: Some(span),
                                    comment: cell.comment,
                                })
                            }
                            Err(_err) => println!("Error parsing Span from {content}!"),
                        }
                    }
                } else if content.starts_with("Sum(") {
                    if let Some(index) = content.find(')') {
                        let span = try_parse_span(&content[4..=index]);
                        match span {
                            // There might be more stuff after position
                            Ok(span) => {
                                let sum = Value::Sum(Box::from(span));
                                updated_cell = Status::Pending(Cell {
                                    original: cell.original,
                                    value: Some(sum),
                                    comment: cell.comment,
                                })
                            }
                            Err(_err) => println!("Error parsing Sum from {content}!"),
                        }
                    }
                }
            }
            Value::Pos(pos) => match context.get(pos) {
                Some(cell) => {
                    updated_cell = cell.clone();
                }
                None => (), // Status::Error
            },
            Value::Sum(span) => {
                // get the list of positions
                let span = match **span {
                    Value::Span(span) => span, // Extract the inner data if it's Value::Span
                    _ => panic!("Expected Value::Span, got something else!"),
                };
                let cell_positions = list_from_span(span);
                let mut ready = true;
                let values: Vec<Value> = cell_positions
                    .into_iter()
                    .filter_map(|pos| match context.get(&pos) {
                        Some(cell) => match cell {
                            Status::Error => None,
                            Status::Empty => None,
                            Status::Pending(cell) => {
                                ready = false;
                                None
                            }
                            Status::Finished(cell) => cell.value.clone(),
                        },
                        None => None,
                    })
                    .collect();
                if ready {
                    let mut sum = 0.0;
                    for value in values {
                        match value {
                            Value::Number(num) => sum += num,
                            cell => println!("Error, tried resolving Sum, thought it was ready, found non-number cell {cell}")
                        }
                    }
                    updated_cell = Status::Finished(Cell {
                        original: cell.original,
                        value: Some(Value::Number(sum)),
                        comment: cell.comment,
                    })
                }
            }
            _ => (),
        }
    }
    if !matches!(updated_cell, Status::Error) {
        let _ = std::mem::replace(pending_cell, updated_cell);
    }
}

fn list_from_span(span: (Pos, Pos)) -> Vec<Pos> {
    let (from, to) = span;
    let mut result = Vec::new();
    for y in from.y..=to.y {
        for x in from.x..=to.x {
            let pos = Pos::new(x as usize, y as usize);
            result.push(pos)
        }
    }
    result
}

fn parse_cell(cell_contents: &str) -> Status {
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
    } else if let Ok(result) = eval_expression(content) {
        Some(result)
    } else if !content.contains(['[']) {
        Some(Value::String(content.to_string()))
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

    /// "[0,0]" => Pos(0,0)
    fn try_parse(from: &str) -> Result<Self, std::num::ParseIntError> {
        let trimmed_brackets = &from[1..from.len() - 1];
        let (y, x) = trimmed_brackets.split_once(',').unwrap();

        Ok(Self {
            x: i32::from_str(x.trim())?,
            y: i32::from_str(y.trim())?,
        })
    }
}
/// "Span([0,0], [1,2])" => Span(Pos(0,0), Pos(1,2))
fn try_parse_span(from: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let trimmed_brackets = &from[5..from.len() - 1];

    if let Some(index) = trimmed_brackets.find(']') {
        let (first, second) = trimmed_brackets.split_at(index + 1);
        // first: "[0,0]", second:", [1,2]"

        return Ok(Value::Span((
            Pos::try_parse(first)?,
            Pos::try_parse(&second[2..])?,
        )));
    }
    Err("Tried to parse Span, no ']' found".into())
}

impl std::fmt::Display for Pos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("Pos[{},{}]", self.x, self.y))
    }
}

#[derive(Debug, Default, Clone)]
enum Status<'a> {
    #[default]
    Error,
    Empty,
    Pending(Cell<'a>),
    Finished(Cell<'a>),
}

#[derive(Debug, Default, Clone)]
struct Cell<'a> {
    original: &'a str, // Original cell content
    value: Option<Value>,
    comment: Option<&'a str>, // Comment, if present
}

#[derive(Debug, Clone)]
enum Value {
    Number(f64),
    String(String),
    Pos(Pos),
    Span((Pos, Pos)),
    Sum(Box<Value>),
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Number(num) => f.write_str(&format!("{num:0.02}")),
            Value::String(string) => f.write_str(string),
            Value::Pos(pos) => f.write_str(&format!("{pos}")),
            Value::Span((from, to)) => f.write_str(&format!("Span({from}, {to})")),
            Value::Sum(span) => f.write_str(&format!("Sum({})", span.as_ref())),
        }
    }
}
