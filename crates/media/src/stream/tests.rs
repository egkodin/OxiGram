use std::sync::mpsc;
use std::thread;

use super::*;

/// A file of `size` bytes (byte `i` is `i as u8`) whose ranges land when
/// the test says, and that records what was asked for.
#[derive(Clone)]
struct Fake(Arc<Mutex<FakeState>>);

struct FakeState {
    size: u64,
    /// Which bytes are on disk.
    present: Vec<bool>,
    requests: Vec<(u64, u64)>,
    is_loading: bool,
    request_fails: bool,
}

impl Fake {
    fn new(size: u64) -> Self {
        Self(Arc::new(Mutex::new(FakeState {
            size,
            present: vec![false; usize::try_from(size).expect("a small test file")],
            requests: Vec::new(),
            is_loading: true,
            request_fails: false,
        })))
    }

    fn land(&self, range: std::ops::Range<usize>) {
        let mut state = self.0.lock().expect("fake lock");
        for byte in &mut state.present[range] {
            *byte = true;
        }
    }

    fn requests(&self) -> Vec<(u64, u64)> {
        self.0.lock().expect("fake lock").requests.clone()
    }

    fn set_loading(&self, is_loading: bool) {
        self.0.lock().expect("fake lock").is_loading = is_loading;
    }
}

impl FileSource for Fake {
    fn size(&self) -> u64 {
        self.0.lock().expect("fake lock").size
    }

    fn ready(&mut self, offset: u64) -> io::Result<u64> {
        let state = self.0.lock().expect("fake lock");
        let start = usize::try_from(offset).expect("a small offset");
        let count = state.present[start.min(state.present.len())..]
            .iter()
            .take_while(|byte| **byte)
            .count();
        Ok(count as u64)
    }

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
        for (index, byte) in buf.iter_mut().enumerate() {
            #[expect(clippy::cast_possible_truncation, reason = "the fake's pattern")]
            let value = (offset + index as u64) as u8;
            *byte = value;
        }
        Ok(buf.len())
    }

    fn request(&mut self, offset: u64, limit: u64) -> io::Result<()> {
        let mut state = self.0.lock().expect("fake lock");
        if state.request_fails {
            return Err(io::Error::other("downloadFile failed"));
        }
        state.requests.push((offset, limit));
        Ok(())
    }

    fn is_loading(&mut self) -> io::Result<bool> {
        Ok(self.0.lock().expect("fake lock").is_loading)
    }
}

fn open(fake: &Fake) -> Stream {
    Stream::new(fake.clone(), Signal::new())
}

#[test]
fn a_range_on_disk_reads_at_once() {
    let fake = Fake::new(4096);
    fake.land(0..4096);
    let mut stream = open(&fake);
    stream.seek(SeekFrom::Start(100)).expect("seeks");
    let mut buf = [0; 4];
    assert_eq!(stream.read(&mut buf).expect("reads"), 4);
    assert_eq!(buf, [100, 101, 102, 103]);
    assert!(fake.requests().is_empty(), "nothing to download");
    assert!(
        (stream.signal().received() - 1.0).abs() < f32::EPSILON,
        "all of it is there"
    );
}

#[test]
fn a_missing_range_is_asked_for_and_waited_on() {
    let fake = Fake::new(1 << 16);
    let mut reader = open(&fake);
    let signal = reader.signal().clone();
    let (done, finished) = mpsc::channel();
    let handle = thread::spawn(move || {
        let mut buf = [0; 8];
        let read = reader.read(&mut buf);
        done.send(()).expect("the test waits");
        (read.map_err(|error| error.to_string()), buf)
    });
    while fake.requests().is_empty() {
        thread::yield_now();
    }
    assert_eq!(fake.requests()[0].0, 0, "from the start");
    assert!(finished.try_recv().is_err(), "the read waits");
    assert!(signal.is_waiting(), "the player can show buffering");
    fake.land(0..16);
    signal.wake();
    let (read, buf) = handle.join().expect("the reader thread ends");
    assert_eq!(read, Ok(8));
    assert_eq!(buf, [0, 1, 2, 3, 4, 5, 6, 7]);
    assert!(!signal.is_waiting());
}

#[test]
fn a_seek_far_ahead_asks_from_there() {
    let fake = Fake::new(1 << 12);
    let size: u64 = 64 << 20;
    fake.0.lock().expect("fake lock").size = size;
    let mut stream = open(&fake);
    stream.signal().set_read_ahead(1 << 20);
    stream.window = Some((0, 1 << 20));
    stream.seek(SeekFrom::End(-1000)).expect("seeks");
    stream.keep_ahead(0).expect("asks");
    assert_eq!(
        fake.requests(),
        vec![(size - 1000, 1000)],
        "the tail, not everything before it"
    );
}

#[test]
fn reading_on_asks_a_few_seconds_ahead_not_the_whole_file() {
    let fake = Fake::new(1 << 16);
    fake.land(0..1 << 16);
    let mut stream = open(&fake);
    stream.signal().set_read_ahead(1 << 10);
    let mut buf = [0; 256];
    assert_eq!(stream.read(&mut buf).expect("reads"), 256);
    assert!(fake.requests().is_empty(), "the next KiB is there");
    let fake = Fake::new(1 << 16);
    fake.land(0..300);
    let mut stream = open(&fake);
    stream.signal().set_read_ahead(1 << 10);
    assert_eq!(stream.read(&mut buf).expect("reads"), 256);
    assert_eq!(fake.requests(), vec![(300, 1 << 10)], "one window past");
    let received = stream.signal().received();
    assert!(
        (received - 300.0 / 65536.0).abs() < f32::EPSILON,
        "{received}"
    );
    assert_eq!(stream.read(&mut [0; 4]).expect("reads"), 4);
    assert_eq!(fake.requests().len(), 1, "the window still covers");
}

#[test]
fn closing_wakes_a_waiting_read() {
    let fake = Fake::new(1 << 16);
    let mut reader = open(&fake);
    let signal = reader.signal().clone();
    let handle = thread::spawn(move || reader.read(&mut [0; 8]).map_err(|error| error.to_string()));
    while !signal.is_waiting() {
        thread::yield_now();
    }
    signal.close();
    let read = handle.join().expect("the reader thread ends");
    assert_eq!(read, Err("the player closed the stream".to_owned()));
}

#[test]
fn a_failed_request_reaches_the_reader() {
    let fake = Fake::new(1 << 16);
    fake.0.lock().expect("fake lock").request_fails = true;
    let read = open(&fake).read(&mut [0; 8]);
    assert_eq!(
        read.map_err(|error| error.to_string()),
        Err("downloadFile failed".to_owned())
    );
}

#[test]
fn a_download_that_stops_short_fails_instead_of_hanging() {
    let fake = Fake::new(1 << 16);
    fake.set_loading(false);
    let mut reader = open(&fake);
    let signal = reader.signal().clone();
    let handle = thread::spawn(move || reader.read(&mut [0; 8]).map_err(|error| error.to_string()));
    for _ in 0..MAX_STALLS {
        signal.wake();
        thread::sleep(Duration::from_millis(20));
    }
    let read = handle.join().expect("the reader thread ends");
    assert_eq!(read, Err("the download stopped short".to_owned()));
    assert_eq!(
        fake.requests().len(),
        usize::try_from(MAX_STALLS).expect("small")
    );
}
