use serde::Deserialize;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;

#[derive(Debug, Deserialize)]
pub struct BingoElement {
    pub content: String,
    pub probability: u32,
}

// The JSON file has the following structure:
// {
//    "bingo_elements": [ { "content": "...", "probability": 0.5 }, ... ]
// }
// We need a wrapper struct to deserialize properly.
#[derive(Debug, Deserialize)]
struct BingoElementsWrapper {
    bingo_elements: Vec<BingoElement>,
}

pub fn read_bingo_elements_from_json(path: &str) -> Result<Vec<BingoElement>, Box<dyn Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let wrapper: BingoElementsWrapper = serde_json::from_reader(reader)?;
    Ok(wrapper.bingo_elements)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_bingo_elements_from_json() {
        let elements = read_bingo_elements_from_json("bingo_elements.json").expect("should read file");
        assert!(!elements.is_empty(), "Expected at least one bingo element");
    }
}

// Example usage:
// let elements = read_bingo_elements_from_json("bingo_elements.json")?;