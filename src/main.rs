use rand::distr::Distribution;

mod bingo_element;
mod board_renderer;

const BOARD_SIZE: usize = 5;
const TOTAL_CELLS: usize = BOARD_SIZE * BOARD_SIZE;

fn main() {
    // read all possible bingo elements from JSON file
    let elements = bingo_element::read_bingo_elements_from_json("bingo_elements.json").unwrap();
    if elements.len() < TOTAL_CELLS {
        panic!("Not enough bingo elements to fill the board");
    }

    // choose 5*5 random elements based on their probabilities
    let mut selected_elements = Vec::new();
    let mut rng = rand::rng();
    let dist = rand::distr::weighted::WeightedIndex::new(
        elements.iter().map(|e| e.probability)
    ).unwrap();
    
    while selected_elements.len() < TOTAL_CELLS {
        let index = dist.sample(&mut rng);

        let element = &elements[index];
        if !selected_elements.contains(&element.content) {
            selected_elements.push(element.content.clone());
        }
    }

    // put the chosen elements into the bingo board
    if let Err(e) = board_renderer::render_board_to_png(&selected_elements, BOARD_SIZE as u32, "bingo_board.png") {
        eprintln!("Failed to render bingo board: {e}");
    } else {
        println!("Bingo board image written to bingo_board.png");
    }
}
