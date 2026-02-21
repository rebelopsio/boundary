Feature: Progress Tracking
  As an engineer refactoring a codebase toward clean architecture
  I want boundary to track architectural scores over time
  So that I can measure progress and prevent regressions from being merged

  Scenario: --track records a snapshot that persists across runs
    Given a valid Go project with a current boundary score of 100
    And no previous snapshot has been recorded
    When I run "boundary check . --track"
    Then a subsequent run of "boundary check . --no-regression" exits 0

  Scenario: --track records a snapshot at a known path
    Given a valid Go project with a current boundary score of 100
    When I run "boundary check . --track"
    Then a snapshot file exists at ".boundary/history.ndjson"

  Scenario Outline: --no-regression does not block the build
    Given a valid Go project with a current boundary score of 100
    And <snapshot context>
    When I run "boundary check . --no-regression"
    Then the exit code is 0

    Examples:
      | snapshot context                              |
      | no previous snapshot has been recorded        |
      | the last recorded snapshot has a score of 75  |
      | the last recorded snapshot has a score of 100 |

  Scenario: --no-regression exits non-zero when the score has dropped
    Given a valid Go project with a current boundary score of 80
    And the last recorded snapshot has a score of 90
    When I run "boundary check . --no-regression"
    Then the exit code is non-zero
    # boundary currently exits 1 for all failures; a dedicated regression
    # exit code (e.g. 2) may be introduced to distinguish regressions from violations

  Scenario: regression report identifies the previous and current scores
    Given a valid Go project with a current boundary score of 80
    And the last recorded snapshot has a score of 90
    When I run "boundary check . --no-regression"
    Then the output includes "90"
    And the output includes "80"

  Scenario: --track appends a new snapshot when combined with --no-regression
    Given a valid Go project with a current boundary score of 100
    And the last recorded snapshot has a score of 100
    When I run "boundary check . --track --no-regression"
    Then the exit code is 0
    And the snapshot history contains 2 entries
