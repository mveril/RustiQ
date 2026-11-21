//! Run MP2 from saved HF orbitals and write a portable snapshot of the applied settings.
use rustiq_core::{
    config::Mp2Config,
    persistence::{PostScfSession, RustiQBundle},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let (Some(input), Some(output), None) = (args.next(), args.next(), args.next()) else {
        return Err("usage: resume_mp2 <input.rustiq> <output.rustiq>".into());
    };

    let bundle = RustiQBundle::open_for_restore(input)?;
    let [entry] = bundle.calculations() else {
        return Err("expected a bundle containing one calculation".into());
    };

    let mut session = PostScfSession::new(entry);
    let mp2 = session.run(&Mp2Config::default())?;
    println!("HF reference: {:?}", mp2.reference);
    println!(
        "MP2 correlation energy: {:.12} Eh",
        mp2.result.correlation_energy
    );
    println!(
        "MP2 total energy: {:.12} Eh",
        mp2.result.electronic_energy + mp2.nuclear_repulsion_energy
    );

    let mut resumed = bundle.with_calculations(vec![mp2.update_snapshot(entry.clone())?])?;
    resumed.write(output)?;
    Ok(())
}
