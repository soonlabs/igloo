mod basic;
pub mod mock;
mod reorg;
mod svm;

fn get_program_path(name: &str) -> String {
    let mut dir = std::env::current_dir().unwrap();
    dir.push("../svm/executor/tests");
    let name = name.replace('-', "_");
    dir.push(name + "_program.so");
    dir.to_str().unwrap().to_string()
}
