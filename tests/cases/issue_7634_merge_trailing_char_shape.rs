//! 관계형: 문단 병합 뒤 글자 모양 구간 위치의 순증가와 붙인 글자의 모양 보존 계약(#7634).
//! 주석 작성: 2026-11-23 13:50 UTC.
//! 렌더링 좌표·시각 일치율을 고정하지 않는다. 독립 근거는 HWP 5.0 사양 표 61의 위치 정의
//! («글자 모양이 바뀌는 시작 위치»)와, 병합이 뒤 문단 글자의 기존 모양을 바꾸지 않아야 한다는
//! 편집 계약이다. 입력은 빈 문서에 공개 편집 API로 만든 합성 문서다(한컴 저장본 아님).
//! 이 변경의 한컴 Print 시각 대조는 이 파일의 검사 범위가 아니다.
#![cfg(not(target_arch = "wasm32"))]

use rhwp::document_core::DocumentCore;
use rhwp::model::paragraph::{CharShapeRef, Paragraph};

fn runs(p: &Paragraph) -> Vec<(u32, u32)> {
    p.char_shapes
        .iter()
        .map(|cs| (cs.start_pos, cs.char_shape_id))
        .collect()
}

fn assert_strictly_increasing(p: &Paragraph) {
    assert!(
        p.char_shapes
            .windows(2)
            .all(|w| w[0].start_pos < w[1].start_pos),
        "char shape positions must strictly increase: {:?}",
        runs(p)
    );
}

fn base_size(core: &DocumentCore, id: u32) -> i32 {
    core.document().doc_info.char_shapes[id as usize].base_size
}

/// 이슈 본문의 재현: 그림 3개 문단(끝 위치에 지운 글자 모양 49)에 그림 2개 문단을 붙인다.
#[test]
fn merge_from_drops_char_shape_left_past_the_end() {
    // 문단의 개체 수만 맞추면 되므로 기본 그림 컨트롤을 쓴다(개체 하나 = 8 UTF-16 단위).
    let picture = || rhwp::model::control::Control::Picture(Box::default());
    let mut head = Paragraph {
        char_count: 25,
        controls: vec![picture(), picture(), picture()],
        ctrl_data_records: vec![None, None, None],
        char_shapes: vec![
            CharShapeRef {
                start_pos: 0,
                char_shape_id: 11,
            },
            CharShapeRef {
                start_pos: 24,
                char_shape_id: 49,
            },
        ],
        has_para_text: true,
        ..Default::default()
    };
    let cover = Paragraph {
        char_count: 17,
        controls: vec![picture(), picture()],
        ctrl_data_records: vec![None, None],
        char_shapes: vec![CharShapeRef {
            start_pos: 0,
            char_shape_id: 22,
        }],
        has_para_text: true,
        ..Default::default()
    };
    head.merge_from(&cover);
    assert_eq!(runs(&head), vec![(0, 11), (24, 22)]);
}

/// 반례: 붙일 문단에 위치 0 구간이 없으면 앞 문단의 끝 구간이 그 앞부분을 계속 덮어야 한다.
#[test]
fn merge_from_keeps_trailing_shape_when_other_has_no_leading_run() {
    let mut head = Paragraph {
        text: "가나".into(),
        char_count: 3,
        char_offsets: vec![0, 1],
        char_shapes: vec![
            CharShapeRef {
                start_pos: 0,
                char_shape_id: 1,
            },
            CharShapeRef {
                start_pos: 2,
                char_shape_id: 2,
            },
        ],
        has_para_text: true,
        ..Default::default()
    };
    let tail = Paragraph {
        text: "다".into(),
        char_count: 2,
        char_offsets: vec![0],
        char_shapes: vec![],
        has_para_text: true,
        ..Default::default()
    };
    head.merge_from(&tail);
    assert_eq!(head.text, "가나다");
    assert_eq!(runs(&head), vec![(0, 1), (2, 2)]);
}

/// 제품 경로: 앞 문단 끝 글자를 다른 모양으로 꾸민 뒤 지우고, 뒤 문단 처음에서 Backspace로
/// 합친다. 붙인 글자는 제 모양을 유지하고 지운 글자의 모양은 남지 않아야 한다.
/// 편집 → 저장 → 다시 열기 → 재저장 → 다시 열기를 HWP·HWPX 각각 검사한다.
#[test]
fn backspace_merge_after_deleting_styled_tail_keeps_merged_text_shape() {
    let mut core = DocumentCore::new_empty();
    core.create_blank_document_native().unwrap();
    core.insert_text_native(0, 0, 0, "가나다").unwrap();
    core.split_paragraph_native(0, 0, 3, None).unwrap();
    core.insert_text_native(0, 1, 0, "라마").unwrap();
    core.apply_char_format_native(0, 0, 2, 3, r#"{"fontSize":1500}"#)
        .unwrap();
    core.apply_char_format_native(0, 1, 0, 2, r#"{"fontSize":2000}"#)
        .unwrap();

    let head = &core.document().sections[0].paragraphs[0];
    let body_shape = head.char_shapes[0].char_shape_id;
    let deleted_shape = head.char_shapes.last().unwrap().char_shape_id;
    let merged_shape = core.document().sections[0].paragraphs[1].char_shapes[0].char_shape_id;
    assert_eq!(base_size(&core, deleted_shape), 1500);
    assert_eq!(base_size(&core, merged_shape), 2000);
    assert_ne!(base_size(&core, body_shape), 1500);

    // "다"를 지우면 그 모양(1500)이 문단 끝 위치에 남는 것이 삭제 경로의 기존 동작이다.
    core.delete_text_native(0, 0, 2, 1).unwrap();
    let head = &core.document().sections[0].paragraphs[0];
    assert_eq!(head.text, "가나");
    // 첫 문단은 구역·단 정의 컨트롤(각 8 UTF-16 단위)을 품으므로 위치를 고정하지 않고
    // 삭제 뒤의 실제 문단 끝 위치를 읽는다.
    let tail_runs = runs(head);
    assert_eq!(tail_runs.len(), 2, "{tail_runs:?}");
    assert_eq!(tail_runs[0], (0, body_shape));
    assert_eq!(tail_runs[1].1, deleted_shape);
    let merge_pos = tail_runs[1].0;

    core.merge_paragraph_native(0, 1).unwrap();
    let check = |core: &DocumentCore| {
        let paras = &core.document().sections[0].paragraphs;
        assert_eq!(paras.len(), 1);
        let p = &paras[0];
        assert_eq!(p.text, "가나라마");
        assert_strictly_increasing(p);
        let sizes: Vec<(u32, i32)> = p
            .char_shapes
            .iter()
            .map(|cs| (cs.start_pos, base_size(core, cs.char_shape_id)))
            .collect();
        // 앞 두 글자는 본문 모양, 붙인 두 글자는 20pt. 지운 글자의 15pt 는 어디에도 없다.
        assert_eq!(sizes.len(), 2, "{sizes:?}");
        assert_eq!(sizes[0].0, 0);
        assert!(sizes[0].1 != 1500 && sizes[0].1 != 2000, "{sizes:?}");
        assert_eq!(sizes[1], (merge_pos, 2000));
    };
    check(&core);

    for save in [
        DocumentCore::export_hwp_native,
        DocumentCore::export_hwpx_native,
    ] {
        let first = DocumentCore::from_bytes(&save(&core).unwrap()).unwrap();
        check(&first);
        let second = DocumentCore::from_bytes(&save(&first).unwrap()).unwrap();
        check(&second);
    }
}
