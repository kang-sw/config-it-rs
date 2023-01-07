
# Tracing modeling





# --- DEPRECATED

- Span은 callsite 단위로 identify
  - 각 callsite마다 최대 N개의 span record를 저장.
    - Span record의 maximum capacity는 동적으로 조절 가능
  - 모든 subspan은 enter()마다 superspan의 fence id를 상속.
  - 사용자는 각 span을 parallel하게 inspect.
    - 이 때, 이름을 클릭해서 해당 span의 history를 조회한다.
    - 상위 span history를 스크롤하면 하위 span도 따라 움직인다.
    - 상위 span capacity가 하위 capacity보다 크면 - 하위 capacity는 공백 처리
  - Sibling span은 사용자가 손으로 정렬 가능하게 -> 순서를 알 수 없음.
- `event`는 로그 취급 ... 각 Span 엔터티에도 표시
  - 일단 발생하면 log에도 한 줄 추가.

---

**Callsite**

단, 하나의 callsite는 여러 parent에 의해 여러 번 다시 호출될 수 있다. Unrelated hierarchy의 subspan을 보여주는 건 pointless ... Parent span slot의 해당하는 subspan slot에 생성된 span 끼워넣기 ... 

핵심은 각 Span 인스턴스는 최대한 경량, Event는 말 그대로 event -> 좀 무거워도 됨

데이터 구조:

- `Span Slot`
  - Recursive list of child span slot ... key: `callsite` -> value: `Span Slot`
  - `VecDeque` for list of created spans
  - Reference to static metadata: Record, Fields, etc.
  - `Values` variant array. Reused by spans
- `Span` -> no dynamic allocation
  - Id
  - Fence
    - 모든 new_span 콜마다 1씩 증가 및 할당.
    - 전송 단계에서 fence를 이용해 최적화(e.g. 몇 번째 fence 이상만 fetch)
    - UI 단에서는 선택한 local root span의 subspan recurse -> 현재 fence ~ 다음 fence 사이의 이벤트를 긁어서 display한다.
  - Start system timestamp, Start Instant, End Instant
  - Local Start Instant, 
  - Using values array range 
  - Front/Last(appender) node to `Arc<Event>`
- `EventMetadata`
  - Static Meta: Callsite/Fields/Name
- `Event: ?Sized` 
  - 생성 시 Root 및 Parent Span의 `Arc<Event>`에 append
  - 타임스탬프
  - `next_span_node`, `next_global_node`
  - 마지막 항목은 Field array ... unsized variable array (note: DST)
 
TODO: Model record_follows_from
TODO: 싹 다시 모델링 ... fence 모델은 re-entrant 처리 못함. A0-> B0-> A1-> B1 ... 


Tracer는 unbounded size의 mpsc로 구현하고, 모든 스레드에서 단일 스레드로 모여들게 설계 ...

