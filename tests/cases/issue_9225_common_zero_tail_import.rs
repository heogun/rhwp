//! 관계형: HWP5 표 CTRL_HEADER의 0 채움 공통 속성 꼬리와 문단 가져오기 수용 계약(#9225).
//! 주석 작성: 2026-11-23 (UTC). 렌더링·조판 결과를 바꾸지 않는 가져오기 검증 계약이므로
//! 시각 일치율·px 좌표를 고정하지 않는다. 입력은 저장소 `samples/hwp_table_test.hwp`의
//! 표 머리에 `00 00` 꼬리를 덧붙여 HWP5로 저장한 뒤 실제 파서로 다시 연 합성 입력이다.
use rhwp::{
    document_core::{DocumentCore, ImportParagraphBlockLimits, ImportParagraphBlockRequest},
    model::control::Control,
};

const SAMPLE: &str = "samples/hwp_table_test.hwp";
const TABLE_PARA: usize = 3;

fn table(core: &DocumentCore, pi: usize) -> &rhwp::model::table::Table {
    match &core.document().sections[0].paragraphs[pi].controls[0] {
        Control::Table(t) => t,
        other => panic!("paragraph {pi} must hold a table, got {other:?}"),
    }
}

/// 샘플 표 머리(설명 길이 0)에 `tail`을 덧붙여 HWP5로 저장하고 실제 파서로 다시 연다.
fn source_with_tail(tail: &[u8]) -> DocumentCore {
    let mut core =
        DocumentCore::from_bytes(&std::fs::read(SAMPLE).expect("repository fixture")).unwrap();
    let raw_len = {
        let Control::Table(t) =
            &mut core.document_mut().sections[0].paragraphs[TABLE_PARA].controls[0]
        else {
            panic!("table")
        };
        assert!(t.common.raw_extra.is_empty(), "fixture starts without tail");
        assert!(
            t.common.description.is_empty(),
            "fixture has no description"
        );
        assert_eq!(t.raw_ctrl_data.len(), 42, "documented 46-byte header");
        t.common.raw_extra = tail.to_vec();
        t.raw_ctrl_data.extend_from_slice(tail);
        t.raw_ctrl_data.len()
    };
    let reopened = DocumentCore::from_bytes(&core.export_hwp_native().unwrap()).unwrap();
    let t = table(&reopened, TABLE_PARA);
    // 저장된 바이트에서 파서가 꼬리를 그대로 읽어야 이 입력이 이슈의 배치를 재현한다.
    assert_eq!(t.common.raw_extra, tail);
    assert_eq!(t.raw_ctrl_data.len(), raw_len);
    reopened
}

fn request() -> ImportParagraphBlockRequest {
    ImportParagraphBlockRequest {
        source_section: 0,
        source_start: TABLE_PARA,
        source_end: TABLE_PARA + 1,
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

fn tables(core: &DocumentCore) -> Vec<&rhwp::model::table::Table> {
    core.document().sections[0]
        .paragraphs
        .iter()
        .flat_map(|p| &p.controls)
        .filter_map(|c| match c {
            Control::Table(t) => Some(t.as_ref()),
            _ => None,
        })
        .collect()
}

fn cell_text(t: &rhwp::model::table::Table) -> Vec<Vec<String>> {
    t.cells
        .iter()
        .map(|c| c.paragraphs.iter().map(|p| p.text.clone()).collect())
        .collect()
}

#[test]
fn zero_filled_common_tail_is_imported_and_round_trips() {
    let source = source_with_tail(&[0, 0]);
    let original = table(&source, TABLE_PARA);
    let before = format!("{:?}", source.document());
    let mut target = blank();
    let preview = target
        .preview_paragraph_block_import_native(source.document(), &request())
        .expect("dry-run accepts zero-filled common tail");
    let result = target
        .import_paragraph_block_native(source.document(), &request())
        .expect("import accepts zero-filled common tail");
    assert_eq!(preview.inserted, result.inserted);
    assert_eq!(result.inserted, 0..2);
    assert_eq!(
        format!("{:?}", source.document()),
        before,
        "source untouched"
    );

    let imported = tables(&target);
    assert_eq!(imported.len(), 2);
    assert_ne!(
        imported[0].common.instance_id,
        imported[1].common.instance_id
    );
    for t in &imported {
        assert_eq!(
            (t.row_count, t.col_count),
            (original.row_count, original.col_count)
        );
        assert_eq!(cell_text(t), cell_text(original));
    }
    for bytes in [
        target.export_hwp_native().unwrap(),
        target.export_hwpx_native().unwrap(),
    ] {
        let reopened = DocumentCore::from_bytes(&bytes).unwrap();
        let saved = tables(&reopened);
        assert_eq!(saved.len(), 2);
        for t in saved {
            assert_eq!(cell_text(t), cell_text(original));
            // 0 채움 꼬리 외의 바이트가 생기지 않는다(HWPX는 꼬리를 쓰지 않는다).
            assert!(t.common.raw_extra.iter().all(|b| *b == 0));
        }
    }
}

#[test]
fn non_zero_common_tail_is_still_rejected_without_mutation() {
    let source = source_with_tail(&[0, 1]);
    let mut target = blank();
    let before = format!("{:?}", target.document());
    for err in [
        target
            .preview_paragraph_block_import_native(source.document(), &request())
            .unwrap_err(),
        target
            .import_paragraph_block_native(source.document(), &request())
            .unwrap_err(),
    ] {
        let message = err.to_string();
        assert!(message.contains("uninterpreted common tail"), "{message}");
        assert!(message.contains("[Paragraph(0), Control(0)]"), "{message}");
    }
    assert_eq!(format!("{:?}", target.document()), before);
}
