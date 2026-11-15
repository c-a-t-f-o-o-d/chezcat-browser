// Link our precompiled artwork resources; normal builds need no resource
// compiler or Cargo dependency. Regenerate them with tools/build_icon_resources.py
// after changing resources/chezcat.rc or assets/chezcat.ico.
use std::{env, path::PathBuf};

#[allow(dead_code)]
#[path = "src/crypto_hash.rs"]
mod crypto_hash;
#[allow(dead_code)]
#[path = "src/license_acceptance.rs"]
mod license_acceptance;
#[allow(dead_code)]
#[path = "src/build_identity.rs"]
mod build_identity;

fn main() {
    println!("cargo:rerun-if-changed=LICENSE");
    println!("cargo:rerun-if-changed=src/license_acceptance.rs");
    println!("cargo:rerun-if-env-changed=CHEZCAT_LICENSE_RECEIPT_DIR");
    println!("cargo:rerun-if-env-changed=LOCALAPPDATA");
    let receipt = license_acceptance::directory(None, None).and_then(|path| {
        // Receipt changes must recheck direct Cargo builds even when source is unchanged.
        println!("cargo:rerun-if-changed={}", path.join("receipt.json").display());
        license_acceptance::require(&path)
    });
    if let Err(problem) = receipt { panic!("{problem} Use python tools/accept_license.py after reviewing LICENSE; this build never accepts silently."); }
    emit_build_identity();

    for file in [
        "assets/chezcat.ico",
        "resources/chezcat.rc",
        "resources/chezcat.res",
        "resources/chezcat-x64.o",
    ] {
        println!("cargo:rerun-if-changed={file}");
    }
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    assert_eq!(
        env::var("CARGO_CFG_TARGET_ARCH").as_deref(),
        Ok("x86_64"),
        "The desktop build currently supports Windows x64 only."
    );
    let name = match env::var("CARGO_CFG_TARGET_ENV").as_deref() {
        Ok("msvc") => "chezcat.res",
        Ok("gnu") => "chezcat-x64.o",
        _ => panic!("Windows builds require the MSVC or GNU Rust target."),
    };
    let resource = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("resources")
        .join(name);
    assert!(
        resource.is_file(),
        "Missing resource file: {}",
        resource.display()
    );
    println!("cargo:rustc-link-arg={}", resource.display());
    // Nested EventTarget callbacks add real interpreter/host frames. Measured
    // validation reaches the existing call guard with this virtual reserve;
    // the old 1 MiB prototype overflowed. Work/time guards stay independent.
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg=/STACK:8388608");
    } else {
        println!("cargo:rustc-link-arg=-Wl,--stack,8388608");
    }
}

// Local build metadata only. This code is not linked into the browser runtime.
fn bounded_git(root: &std::path::Path, args: &[&str], maximum: usize) -> Option<Vec<u8>> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};
    let mut child = Command::new("git")
        .args(["--no-optional-locks", "--literal-pathspecs", "-c", "core.fsmonitor=false", "-c", "core.untrackedCache=false"])
        .args(args).current_dir(root).env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null()).stderr(Stdio::null()).stdout(Stdio::piped()).spawn().ok()?;
    let Some(stdout)=child.stdout.take() else {let _=child.kill();let _=child.wait();return None;};
    let (send,receive)=mpsc::sync_channel(1);
    let reader=match std::thread::Builder::new().name("build-identity-local-git".into()).spawn(move || {
        let mut bytes=Vec::new();
        let result=stdout.take(maximum.saturating_add(1) as u64).read_to_end(&mut bytes)
            .ok().filter(|_|bytes.len()<=maximum).map(|_|bytes);
        let _=send.send(result);
    }) {Ok(reader)=>reader,Err(_)=>{let _=child.kill();let _=child.wait();return None;}};
    let deadline=Instant::now()+Duration::from_secs(2);
    let mut output=None;
    let result=loop {
        match receive.try_recv() {
            Ok(Some(bytes))=>output=Some(bytes),
            Ok(None)=>break None,
            Err(mpsc::TryRecvError::Disconnected) if output.is_none()=>break None,
            _=>{}
        }
        match child.try_wait() {
            Ok(Some(status))=>{
                if !status.success() {break None;}
                if let Some(bytes)=output.take() {break Some(bytes);}
                let remaining=deadline.saturating_duration_since(Instant::now());
                break receive.recv_timeout(remaining).ok().flatten();
            }
            Ok(None)=>{},Err(_)=>break None,
        }
        if Instant::now()>=deadline {break None;}
        std::thread::sleep(Duration::from_millis(5));
    };
    // On every failure reap this exact owned local Git process. No shell,
    // remote operation, hook, credential helper or recursive submodule walk.
    if result.is_none() {let _=child.kill();}
    let _=child.wait();
    // A completed bounded read has already closed its pipe. Do not wait on a
    // potentially blocked reader after timeout; build-process exit owns it.
    if output.is_some() || result.is_some() {let _=reader.join();}
    result
}
fn small_text(path:&std::path::Path,maximum:u64)->Option<String> {
    use std::io::Read;
    let file=std::fs::File::open(path).ok()?;
    if file.metadata().ok()?.len()>maximum {return None;}
    let mut text=String::new();file.take(maximum.saturating_add(1)).read_to_string(&mut text).ok()?;
    (text.len() as u64<=maximum).then_some(text)
}
fn watch(path:&std::path::Path)->bool {
    let Some(text)=path.to_str().filter(|text|!text.contains('\r')&&!text.contains('\n')) else {return false;};
    println!("cargo:rerun-if-changed={text}");true
}
fn watch_git_state(root:&std::path::Path)->bool {
    let entry=root.join(".git");
    let git_dir=if entry.is_dir() {entry} else {
        watch(&entry);
        let Some(text)=small_text(&entry,4096) else {return false;};
        let Some(value)=text.trim().strip_prefix("gitdir: ") else {return false;};
        root.join(value)
    };
    let common=small_text(&git_dir.join("commondir"),4096).map_or_else(||git_dir.clone(),|text|git_dir.join(text.trim()));
    // Cargo treats a missing watched file as perpetually changed. Watch the
    // existing refs directory to observe creation/removal of loose refs, and
    // only an existing packed-refs file; HEAD/index stay required inputs.
    let refs=common.join("refs");
    let mut okay=watch(&git_dir.join("HEAD")) && watch(&git_dir.join("index"))
        && refs.is_dir() && watch(&refs);
    let packed=common.join("packed-refs");
    if packed.is_file() {okay &= watch(&packed);}
    if let Some(head)=small_text(&git_dir.join("HEAD"),4096) {
        if let Some(reference)=head.trim().strip_prefix("ref: ") {
            let path=std::path::Path::new(reference);
            if reference.starts_with("refs/") && path.components().all(|part|matches!(part,std::path::Component::Normal(_))) {
                let loose=common.join(path);
                if loose.is_file() {okay &= watch(&loose);}
            } else {okay=false;}
        }
    } else {okay=false;}
    okay
}
fn emit_build_identity() {
    use build_identity::{SourceOrigin, TreeState};
    let root=PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo source root is unavailable."));
    println!("cargo:rerun-if-env-changed=CHEZCAT_DECLARED_BUILD_TAG");
    for name in ["src","SOURCE_REVISION","Cargo.toml","Cargo.lock","THIRD_PARTY_NOTICES.md"] {watch(&root.join(name));}
    // An exported source folder inside a different repository must not adopt
    // that parent's source SHA, private ancestry or working-tree state.
    let own_repository=bounded_git(&root,&["rev-parse","--show-toplevel"],4096)
        .and_then(|raw|String::from_utf8(raw).ok())
        .and_then(|path|std::fs::canonicalize(path.trim()).ok())
        .zip(std::fs::canonicalize(&root).ok()).is_some_and(|(git_root,root)|git_root==root);
    let mut tracked_watch=false;
    if let Some(paths)=own_repository.then(||bounded_git(&root,&["ls-files","-v","-z"],256*1024)).flatten() {
        tracked_watch=true;
        for raw in paths.split(|byte|*byte==0).filter(|raw|!raw.is_empty()) {
            let Some(path)=build_identity::tracked_path(raw) else {tracked_watch=false;continue;};
            tracked_watch &= watch(&root.join(path));
        }
    }
    let watched=own_repository && watch_git_state(&root) && tracked_watch && !root.join(".gitmodules").exists();
    let head=own_repository.then(||bounded_git(&root,&["rev-parse","--verify","HEAD"],128)).flatten().and_then(|raw|String::from_utf8(raw).ok());
    let head=head.as_deref().map(str::trim).filter(|text|build_identity::revision(text).is_some());
    let status=head.and_then(|_|bounded_git(&root,&["status","--porcelain=v1","--untracked-files=normal","--ignore-submodules=all"],64*1024));
    let final_head=head.and_then(|_|bounded_git(&root,&["rev-parse","--verify","HEAD"],128)).and_then(|raw|String::from_utf8(raw).ok());
    let exported=small_text(&root.join("SOURCE_REVISION"),256);
    let (source_revision,origin,tree)=if let Some(head)=head.filter(|head|final_head.as_deref().map(str::trim)==Some(*head)) {
        let tree=match status {Some(ref bytes) if !bytes.is_empty()=>"dirty",Some(_) if watched=>"clean",_=>"unknown"};
        (head,"git",tree)
    } else if let Some(revision)=exported.as_deref().map(str::trim).filter(|text|build_identity::revision(text).is_some()) {
        (revision,"exported","unknown")
    } else {("","unknown","unknown")};
    let source=build_identity::source(Some(source_revision),Some(origin),Some(tree));
    let declared_tag=env::var("CHEZCAT_DECLARED_BUILD_TAG").ok();
    let identity=build_identity::Identity::new(build_identity::PRODUCT_VERSION,source,declared_tag.as_deref())
        .unwrap_or_else(|reason|panic!("{reason}"));
    println!("cargo:rustc-env=CHEZCAT_EMBEDDED_SOURCE_REVISION={}",identity.source().revision().unwrap_or(""));
    println!("cargo:rustc-env=CHEZCAT_EMBEDDED_SOURCE_ORIGIN={}",match source.origin(){SourceOrigin::Git=>"git",SourceOrigin::Exported=>"exported",SourceOrigin::Unknown=>"unknown"});
    println!("cargo:rustc-env=CHEZCAT_EMBEDDED_TREE_STATE={}",match source.tree(){TreeState::Clean=>"clean",TreeState::Dirty=>"dirty",TreeState::Unknown=>"unknown"});
    println!("cargo:rustc-env=CHEZCAT_EMBEDDED_DECLARED_TAG={}",declared_tag.as_deref().unwrap_or(""));
}
