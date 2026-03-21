mod data;
mod model;
#[allow(dead_code)]
mod tensor;
mod tokenizer;

use model::{Config, GPT, Gradients};
use std::io::Write;
use std::time::Instant;

fn count_params(config: &Config) -> usize {
    let e = config.n_embd;
    let v = config.vocab_size;
    let nl = config.n_layer;
    let inner = 4 * e;

    let mut total = 0;
    total += v * e;              // token_emb
    total += config.block_size * e; // pos_emb
    for _ in 0..nl {
        total += e;              // ln1_gamma
        total += e;              // ln1_beta
        total += e * 3 * e;     // qkv_w
        total += e * e;          // attn_proj
        total += e;              // ln2_gamma
        total += e;              // ln2_beta
        total += e * inner;      // ff_w1
        total += inner;          // ff_b1
        total += inner * e;      // ff_w2
        total += e;              // ff_b2
    }
    total += e;                  // ln_f_gamma
    total += e;                  // ln_f_beta
    total += e * v;              // lm_head
    total
}

/// Print ASCII loss chart
fn print_loss_chart(losses: &[(usize, f32)]) {
    if losses.is_empty() { return; }

    let max_loss = losses.iter().map(|&(_, l)| l).fold(0.0f32, f32::max);
    let min_loss = losses.iter().map(|&(_, l)| l).fold(f32::MAX, f32::min);
    let chart_width = 50;

    println!("\n=== Training Loss Curve ===\n");
    println!("  {:>6.2} |{}", max_loss, " ".repeat(chart_width));

    let chart_height = 20;
    for row in 0..chart_height {
        let threshold = max_loss - (max_loss - min_loss) * (row as f32 / (chart_height - 1) as f32);
        let label = if row == 0 || row == chart_height - 1 || row == chart_height / 2 {
            format!("{:>6.2}", threshold)
        } else {
            "      ".to_string()
        };

        let mut line = String::new();
        // Map each loss to a column position
        let num_points = losses.len();
        let mut cols = vec![' '; chart_width];

        for (idx, &(_, loss)) in losses.iter().enumerate() {
            let col = (idx as f32 / num_points as f32 * chart_width as f32) as usize;
            let col = col.min(chart_width - 1);
            let loss_row = ((max_loss - loss) / (max_loss - min_loss) * (chart_height - 1) as f32) as usize;
            if loss_row == row {
                cols[col] = '█';
            }
        }

        for c in &cols {
            line.push(*c);
        }
        println!("  {} |{}", label, line);
    }

    println!("  {:>6.2} |{}", min_loss, "_".repeat(chart_width));
    println!("         0{:>width$}{}", "", losses.last().unwrap().0, width = chart_width - 5);
    println!("                        Step\n");
}

fn main() {
    println!("=== Mini GPT - Pure Rust Transformer ===");
    println!("    Pure Rust, no ML frameworks, full backprop\n");

    // 1. Load training data and build tokenizer
    let text = data::get_training_data();
    let tok = tokenizer::Tokenizer::from_text(text);
    let encoded = tok.encode(text);

    println!("Training text: {} chars, {} tokens", text.len(), encoded.len());
    println!("Vocabulary size: {}", tok.vocab_size());

    // 2. Create model
    let block_size = 48;
    let config = Config {
        vocab_size: tok.vocab_size(),
        n_embd: 48,
        n_head: 4,
        n_layer: 3,
        block_size,
    };

    let num_params = count_params(&config);
    println!(
        "Model: {} layers, {} embd, {} heads, block_size={}, params={}",
        config.n_layer, config.n_embd, config.n_head, config.block_size, num_params
    );

    let mut model = GPT::new(config.clone());

    // 3. Training loop
    let mut rng = rand::thread_rng();
    let batch_size = 16;
    let num_steps = 2500;
    let lr = 0.001;

    println!("\nTraining for {} steps (batch_size={}, lr={})...\n", num_steps, batch_size, lr);
    let start_time = Instant::now();

    let mut loss_log: Vec<(usize, f32, f32)> = Vec::new(); // (step, loss, time)

    for step in 0..num_steps {
        // Create batch
        let (inputs, targets) =
            data::create_batches(&encoded, block_size, batch_size, &mut rng);

        // Accumulate gradients over batch
        let mut total_loss = 0.0f32;
        let mut batch_grads = Gradients::zero_like(&config);

        for b in 0..batch_size {
            let (loss, grads) = model.forward_backward(&inputs[b], &targets[b]);
            total_loss += loss;
            batch_grads.accumulate(&grads);
        }

        // Average gradients
        batch_grads.scale(1.0 / batch_size as f32);
        let avg_loss = total_loss / batch_size as f32;

        // Apply gradients with Adam
        model.apply_gradients(&batch_grads, lr);

        let elapsed = start_time.elapsed().as_secs_f32();

        // Log every 10 steps for the chart
        if step % 10 == 0 {
            loss_log.push((step, avg_loss, elapsed));
        }

        if step % 100 == 0 || step == num_steps - 1 {
            println!("Step {:4} | Loss: {:.4} | Time: {:.1}s", step, avg_loss, elapsed);
        }
    }

    let total_time = start_time.elapsed().as_secs_f32();
    println!("\nTraining complete in {:.1}s", total_time);

    // Save training log CSV
    if let Ok(mut file) = std::fs::File::create("training_log.csv") {
        let _ = writeln!(file, "step,loss,time_seconds");
        for &(step, loss, time) in &loss_log {
            let _ = writeln!(file, "{},{:.6},{:.2}", step, loss, time);
        }
        println!("Training log saved to training_log.csv");
    }

    // Print ASCII loss chart
    let chart_data: Vec<(usize, f32)> = loss_log.iter().map(|&(s, l, _)| (s, l)).collect();
    print_loss_chart(&chart_data);

    // 4. Generate text
    println!("=== Generated Text ===\n");

    for seed in &["the ", "and ", "to ", "of "] {
        let seed_tokens = tok.encode(seed);
        let generated_tokens = model.generate(&seed_tokens, 200);
        let generated_text = tok.decode(&generated_tokens);
        println!("Seed: {:?}", seed);
        println!("{}\n", generated_text);
    }

    println!("=== Generation Complete ===");
}
