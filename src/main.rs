use axum::{extract::Path, routing::get, Json, Router};
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::io;
use std::io::Read;
use std::io::Write;
use std::path::Path as p;
use std::sync::Arc;
use std::time::Instant;
use tokio;
use bincode;
fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len()<3 {return;}
    if &args[1] == "build" {
        if args.len()<4 {return;}
        let big_path = p::new(&args[2]);
        let big_file = read_file(big_path);
        println!("Building corpus...");
        let start_time = Instant::now();
        let (words, n) = get_words(big_file);
        let elapsed_time = start_time.elapsed();
        println!("Elapsed time: {:?}", elapsed_time);
        let mut file = fs::File::create(&args[3]).unwrap();
        let _ = file.write_all(&bincode::serialize(&words).unwrap());
        println!(
            "Corpus built. {} words found, {} non-unique",
            words.len(),
            n
        );
        return ;
    }
    let mut file = fs::File::open(&args[2]).unwrap();
    let mut bytes = Vec::new();
    let _ = file.read_to_end(&mut bytes);
    let words:HashMap<String, isize> = bincode::deserialize(&bytes).unwrap();
    if &args[1] == "cli" {
        println!("Enter ! to exit");
        println!("Enter terms for correction");
        loop {
            let mut word: String = String::new();

            io::stdin()
                .read_line(&mut word)
                .expect("Failed to read line");
            word = word.trim().to_string();

            if word == "!" {
                break;
            }
            println!(
                "{}",
                correct_word(&word, &words).get("correct_word").unwrap()
            );
        }
    } else if &args[1] == "ws" {
        
        serve(words);
    }
}

#[tokio::main]
async fn serve(words: HashMap<String, isize>) {
    println!("Web Service started at localhost:3005");
    let shared_state = Arc::new(words);
    let app = Router::new().route(
        "/correct/:word",
        get({
            let shared_state = Arc::clone(&shared_state);
            move |path| handle_get_correct(path, shared_state)
        }),
    );
    axum::Server::bind(&"0.0.0.0:3005".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
    
}

async fn handle_get_correct(
    Path(word): Path<String>,
    words: Arc<HashMap<String, isize>>,
) -> Json<Value> {
    correct_word(&word, &words)
}

fn read_file(file_path: &p) -> String {
    let contents = match fs::read_to_string(file_path) {
        Ok(conts) => conts,
        Err(e) => {
            eprintln!("{e}");
            String::from("")
        }
    };
    return contents;
}

fn get_words(file: String) -> (HashMap<String, isize>, isize) {
    let re = Regex::new(r"([A-za-z]+)").unwrap();
    let mut words: HashMap<String, isize> = HashMap::new();
    let mut n = 0;
    for (_, [w]) in re.captures_iter(&file).map(|c| c.extract()) {
        words
            .entry(w.to_string().replace("_", "").to_lowercase())
            .and_modify(|count| *count += 1)
            .or_insert(1);
        n += 1;
    }
    (words, n)
}

fn probability(words: &HashMap<String, isize>, word: &String) -> isize {
    return words[word];
}

fn known_alts(alts: &HashSet<String>, words: &HashMap<String, isize>) -> HashSet<String> {
    let mut known: HashSet<String> = HashSet::new();
    for w in alts {
        if words.contains_key(w) {
            known.insert(w.clone());
        }
    }
    known
}

fn one_edit_words(word: &String) -> HashSet<String> {
    let mut edits: HashSet<String> = HashSet::new();
    let letters = "abcdefghijklmnopqrstuvwxyz";
    let mut splits: Vec<(String, String)> = Vec::new();
    for i in 0..word.len() + 1 {
        splits.push((word[..i].to_string().clone(), word[i..].to_string().clone()));
    }

    for (l, r) in &splits {
        if r != "" {
            edits.insert([&l, &r[1..]].join(""));
        }
        if r.len() > 1 {
            edits.insert([&l, &r[1..2], &r[0..1], &r[2..]].join(""));
        }
        if r != "" {
            for c in letters.chars() {
                edits.insert([&l, c.to_string().as_str(), &r[1..]].join(""));
            }
        }
        for c in letters.chars() {
            edits.insert([&l, c.to_string().as_str(), &r].join(""));
        }
    }

    edits
}

fn two_edit_words(word: &String) -> HashSet<String> {
    let mut edits: HashSet<String> = HashSet::new();
    for e1 in one_edit_words(word) {
        for e2 in one_edit_words(&e1) {
            edits.insert(e2);
        }
    }
    edits
}

fn correct_word(word: &String, words: &HashMap<String, isize>) -> Json<Value> {
    let start_time = Instant::now();
    if words.contains_key(word){
        return Json(json!({ "correct_word": word, "edits": 0, "found": true }))
    }
    let one_edit = known_alts(&one_edit_words(&word), &words);
    if one_edit.len() > 0 {
        let w = one_edit
            .iter()
            .max_by(|a, b| probability(words, a).cmp(&probability(words, b)))
            .unwrap()
            .clone();
        let elapsed_time = start_time.elapsed();
        println!("Elapsed time: {:?}", elapsed_time);
        return Json(json!({ "correct_word": w, "edits": 1, "found": true }));
    }
    let two_edit = known_alts(&two_edit_words(word), &words);
    if two_edit.len() > 0 {
        let w = two_edit
            .iter()
            .max_by(|a, b| probability(words, a).cmp(&probability(words, b)))
            .unwrap()
            .clone();
        let elapsed_time = start_time.elapsed();
        println!("Elapsed time: {:?}", elapsed_time);
        return Json(json!({ "correct_word": w, "edits": 2, "found": true }));
    }
    let elapsed_time = start_time.elapsed();
    println!("Elapsed time: {:?}", elapsed_time);
    return Json(json!({ "correct_word": word.clone(), "edits": 1, "found": false }));
}
