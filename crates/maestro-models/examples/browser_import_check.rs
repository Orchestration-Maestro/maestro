use maestro_models::{complete, get_model};

fn main() {
    let model = get_model("google", "gemini-2.5-flash");
    std::hint::black_box((model, complete));
}
