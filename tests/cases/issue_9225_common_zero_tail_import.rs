//! 관계형: HWP5 개체 CTRL_HEADER의 0 채움 공통 속성 꼬리와 문단 가져오기 수용 계약(#9225).
//! 주석 작성: 2026-11-23 10:15 UTC, 실물 샘플 전환 2026-11-23 UTC.
//! 렌더링·조판 결과를 바꾸지 않는 가져오기 검증 계약이므로 시각 일치율·px 좌표를 고정하지 않는다.
//! 입력 근거: 저장소 HWP5 939건을 CFB 레코드 수준에서 조사하면 `00 00` 꼬리는 버전 5.1.1.0
//! 파일에서만(표 1554·수식 24893건) 관찰되고 0이 아닌 꼬리는 없다(사양 정오표 #42).
//! 표: `samples/셀보호.hwp` (SHA-256 1416f36e…), 수식: `samples/수식-문자처럼취급-아님.hwp`
//! (SHA-256 52b883d8…) — 둘 다 한컴 저장본이며 꼬리를 덧붙이지 않은 원본 그대로 연다.
//! 0이 아닌 꼬리는 실물 사례가 없어 표 샘플의 꼬리만 `00 01`로 바꿔 저장한 합성 입력으로 검사한다.
use rhwp::{
    document_core::{DocumentCore, ImportParagraphBlockLimits, ImportParagraphBlockRequest},
    model::control::Control,
};

const TABLE_SAMPLE: &str = "samples/셀보호.hwp";
const EQUATION_SAMPLE: &str = "samples/수식-문자처럼취급-아님.hwp";

fn open(path: &str) -> DocumentCore {
    DocumentCore::from_bytes(&std::fs::read(path).expect("required repository fixture")).unwrap()
}

fn is_target(c: &Control) -> bool {
    matches!(c, Control::Table(_) | Control::Equation(_))
}

fn common_tail(c: &Control) -> &[u8] {
    match c {
        Control::Table(t) => &t.common.raw_extra,
        Control::Equation(e) => &e.common.raw_extra,
        other => panic!("unexpected control {other:?}"),
    }
}

fn instance_id(c: &Control) -> u32 {
    match c {
        Control::Table(t) => t.common.instance_id,
        Control::Equation(e) => e.common.instance_id,
        other => panic!("unexpected control {other:?}"),
    }
}

/// 비교용 내용: 표는 행·열·셀 텍스트, 수식은 스크립트.
fn content(c: &Control) -> String {
    match c {
        Control::Table(t) => format!(
            "{}x{} {:?}",
            t.row_count,
            t.col_count,
            t.cells
                .iter()
                .map(|cell| cell.paragraphs.iter().map(|p| p.text.clone()).collect())
                .collect::<Vec<Vec<String>>>()
        ),
        Control::Equation(e) => e.script.clone(),
        other => panic!("unexpected control {other:?}"),
    }
}

/// 첫 대상 개체를 가진 최상위 문단 번호.
fn host_paragraph(core: &DocumentCore) -> usize {
    core.document().sections[0]
        .paragraphs
        .iter()
        .position(|p| p.controls.iter().any(is_target))
        .expect("fixture holds a table or equation")
}

fn targets(core: &DocumentCore) -> Vec<&Control> {
    core.document().sections[0]
        .paragraphs
        .iter()
        .flat_map(|p| &p.controls)
        .filter(|c| is_target(c))
        .collect()
}

fn request(source: usize) -> ImportParagraphBlockRequest {
    ImportParagraphBlockRequest {
        source_section: 0,
        source_start: source,
        source_end: source + 1,
        target_section: 0,
        insert_before: 0,
        count: 2,
        limits: ImportParagraphBlockLimits::default(),
    }
}

fn blank() -> DocumentCore {
    let mut c = DocumentCore::new_empty();
    c.create_blank_document_native().unwrap();
    c
}

fn assert_imports_and_round_trips(path: &str) {
    let source = open(path);
    let pi = host_paragraph(&source);
    let originals: Vec<&Control> = source.document().sections[0].paragraphs[pi]
        .controls
        .iter()
        .filter(|c| is_target(c))
        .collect();
    // 실물 저장본이 이슈의 배치(설명 길이 0 + `00 00`)를 가진다는 전제를 먼저 확인한다.
    for c in &originals {
        assert_eq!(common_tail(c), [0, 0], "{path}: real zero-filled tail");
    }
    let before = format!("{:?}", source.document());

    let mut target = blank();
    let preview = target
        .preview_paragraph_block_import_native(source.document(), &request(pi))
        .unwrap_or_else(|e| panic!("{path}: dry-run must accept zero tail: {e}"));
    let result = target
        .import_paragraph_block_native(source.document(), &request(pi))
        .unwrap_or_else(|e| panic!("{path}: import must accept zero tail: {e}"));
    assert_eq!(preview.inserted, result.inserted);
    assert_eq!(result.inserted, 0..2);
    assert_eq!(format!("{:?}", source.document()), before, "{path}: source");

    let check = |core: &DocumentCore, tail: &[u8]| {
        let copies = targets(core);
        assert_eq!(copies.len(), originals.len() * 2, "{path}");
        let ids: std::collections::BTreeSet<_> = copies.iter().map(|c| instance_id(c)).collect();
        assert_eq!(ids.len(), copies.len(), "{path}: distinct instance ids");
        for (i, c) in copies.iter().enumerate() {
            assert_eq!(
                content(c),
                content(originals[i % originals.len()]),
                "{path}"
            );
            assert_eq!(common_tail(c), tail, "{path}");
        }
    };
    // 가져온 사본은 원본 꼬리를 그대로 보존한다.
    check(&target, &[0, 0]);

    // 편집 → 저장 → 다시 열기 → 재저장 → 다시 열기를 형식별로 검사한다.
    // HWP5는 꼬리를 그대로 다시 쓰고, HWPX 저장은 꼬리를 쓰지 않는다.
    let save_hwp = |c: &DocumentCore| c.export_hwp_native().unwrap();
    let save_hwpx = |c: &DocumentCore| c.export_hwpx_native().unwrap();
    for (save, tail) in [
        (
            &save_hwp as &dyn Fn(&DocumentCore) -> Vec<u8>,
            &[0u8, 0][..],
        ),
        (&save_hwpx, &[][..]),
    ] {
        let first = DocumentCore::from_bytes(&save(&target)).unwrap();
        let second = DocumentCore::from_bytes(&save(&first)).unwrap();
        check(&first, tail);
        check(&second, tail);
    }
}

#[test]
fn real_table_with_zero_filled_common_tail_imports_and_round_trips() {
    assert_imports_and_round_trips(TABLE_SAMPLE);
}

#[test]
fn real_equation_with_zero_filled_common_tail_imports_and_round_trips() {
    assert_imports_and_round_trips(EQUATION_SAMPLE);
}

#[test]
fn non_zero_common_tail_is_still_rejected_without_mutation() {
    let mut core = open(TABLE_SAMPLE);
    let pi = host_paragraph(&core);
    {
        let para = &mut core.document_mut().sections[0].paragraphs[pi];
        let Some(Control::Table(t)) = para.controls.iter_mut().find(|c| is_target(c)) else {
            panic!("table")
        };
        assert_eq!(t.common.raw_extra, [0, 0]);
        let n = t.raw_ctrl_data.len();
        assert_eq!(t.raw_ctrl_data[n - 2..], [0, 0]);
        t.common.raw_extra = vec![0, 1];
        t.raw_ctrl_data[n - 1] = 1;
    }
    // 저장된 바이트에서 파서가 0이 아닌 꼬리를 그대로 읽는지 확인한다.
    let source = DocumentCore::from_bytes(&core.export_hwp_native().unwrap()).unwrap();
    let tail = source.document().sections[0].paragraphs[pi]
        .controls
        .iter()
        .find(|c| is_target(c))
        .map(common_tail)
        .unwrap()
        .to_vec();
    assert_eq!(tail, [0, 1]);

    let mut target = blank();
    let before = format!("{:?}", target.document());
    for err in [
        target
            .preview_paragraph_block_import_native(source.document(), &request(pi))
            .unwrap_err(),
        target
            .import_paragraph_block_native(source.document(), &request(pi))
            .unwrap_err(),
    ] {
        let message = err.to_string();
        assert!(message.contains("uninterpreted common tail"), "{message}");
    }
    assert_eq!(format!("{:?}", target.document()), before);
}
