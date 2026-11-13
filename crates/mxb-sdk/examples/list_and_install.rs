use mxb_sdk::core::{folders, install, mods};

fn main() -> mxb_sdk::core::Result<()> {
    let source = std::env::args().nth(1).expect("pass a mod folder");
    let game = folders::find_installs().into_iter().next().expect("MX Bikes was not found");

    for bike in mods::list(&game.mods_dir, Some(mods::Kind::Bikes)) {
        println!("{}", bike.name);
    }
    let plan = install::plan(source, &game.mods_dir, None)?;
    for write in plan.writes() {
        println!("will write {}", write.destination.display());
    }
    println!("installed {} files", install::apply(&plan)?.written.len());
    Ok(())
}

