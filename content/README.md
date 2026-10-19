# 숨은 메뉴 내용 고치기 (관리자용)

`content.json` 을 고쳐 **main 에 올리면**, MAGO 를 켜 둔 모든 사람이 **6시간 안에(또는 다음에 켤 때)** 새 내용을 받습니다. 프로그램을 다시 깔 필요가 없습니다.

## 꼭 지킬 것
1. **`version` 을 하나 올립니다.** 숫자가 그대로면 바뀐 것으로 보지 않습니다.
2. `id` 는 한 번 정하면 바꾸지 않습니다. 사람들이 "찾은 것"을 id 로 기억합니다. 바꾸면 찾은 기록이 사라진 것처럼 보입니다.
3. 링크는 `https://` 만 됩니다. 틀린 파일(쉼표 빠짐 등)은 프로그램이 받지 않고 이전 내용을 계속 씁니다.

## 숨은 메뉴 하나
```json
{
  "id": "trash_note",
  "name": "쓰레기통의 쪽지",
  "unlock": { "type": "feedTotal", "count": 5 },
  "reward": {
    "title": "오늘의 글",
    "body": "쪽지에 보일 글. 줄바꿈은 \n",
    "link": "https://crownyclass.com/community.html?board=hangout",
    "linkLabel": "자세히 보기"
  }
}
```

| 여는 조건 `unlock.type` | 뜻 |
|---|---|
| `feedTotal` + `count` | 쓰레기통에 먹인 횟수가 count 번이 되면 |
| `feedBatch` + `count` | 한 번에 count 개 넘게 먹이면 |
| `hour` + `from` · `to` | 메뉴를 연 시각이 from시 이상 to시 미만이면 (0~24) |

- `link` · `linkLabel` 은 없어도 됩니다 (그러면 쪽지만 보이고 닫기만 있습니다).
- 고리 가운데의 "숨은 메뉴 N개"는 이 목록 수에서 그 사람이 찾은 수를 뺀 것입니다.
- 새 조건 종류(예: 특정 파일 이름)는 프로그램을 고쳐야 합니다 — `src-tauri/src/content.rs` 의 `Unlock`.
