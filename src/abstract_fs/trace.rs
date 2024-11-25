use std::num::ParseIntError;

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct Trace {
    pub rows: Vec<TraceRow>,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct TraceRow {
    index: u32,
    command: String,
    return_code: i32,
    errno: String,
}

type Result<T> = std::result::Result<T, TraceError>;

#[derive(Debug, PartialEq)]
pub enum TraceError {
    EmptyTrace,
    InvalidColumnsCount,
    IntParse(ParseIntError),
}

impl From<ParseIntError> for TraceError {
    fn from(err: ParseIntError) -> TraceError {
        TraceError::IntParse(err)
    }
}

pub fn parse_trace(trace: String) -> Result<Trace> {
    let lines: Vec<&str> = trace.split('\n').collect();
    if lines.len() <= 1 {
        return Err(TraceError::EmptyTrace);
    }
    let mut trace = Trace { rows: vec![] };
    for line in &lines[1..] {
        if line.trim().is_empty() {
            break;
        }
        let columns: Vec<&str> = line.split(",").collect();
        if columns.len() != 4 {
            return Err(TraceError::InvalidColumnsCount);
        }
        let index = columns[0].trim().parse()?;
        let command = columns[1].trim().to_owned();
        let return_code = columns[2].trim().parse()?;
        let errno: String = columns[3].trim().to_owned();
        trace.rows.push(TraceRow {
            index,
            command,
            return_code,
            errno,
        });
    }
    Ok(trace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_trace() {
        assert_eq!(Err(TraceError::EmptyTrace), parse_trace("".to_owned()))
    }

    #[test]
    fn test_header_only() {
        assert_eq!(
            Ok(Trace { rows: vec![] }),
            parse_trace("Index,Command,ReturnCode,Errno\n".to_owned())
        )
    }

    #[test]
    fn test_invalid_columns_count() {
        let trace = r#"
Index,Command,ReturnCode,Errno
    1,    Foo,        42,Success(0), ???
    2,    Bar,        -1,Error(42)
"#
        .trim();
        assert_eq!(
            Err(TraceError::InvalidColumnsCount),
            parse_trace(trace.to_owned())
        )
    }

    #[test]
    fn test_ok_trace() {
        let trace = r#"
Index,Command,ReturnCode,Errno
    1,    Foo,        42,Success(0)
    2,    Bar,        -1,Error(42)
"#
        .trim();
        assert_eq!(
            Ok(Trace {
                rows: vec![
                    TraceRow {
                        index: 1,
                        command: "Foo".to_owned(),
                        return_code: 42,
                        errno: "Success(0)".to_owned(),
                    },
                    TraceRow {
                        index: 2,
                        command: "Bar".to_owned(),
                        return_code: -1,
                        errno: "Error(42)".to_owned(),
                    },
                ]
            }),
            parse_trace(trace.to_owned())
        )
    }
}
