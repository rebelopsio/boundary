Feature: Diagram Generation (FR-13)
  As a developer
  I want to generate architecture diagrams from my codebase
  So that I can visualize component relationships and layer structure

  Scenario: Mermaid layer diagram does not contain synthetic file nodes
    Given a Go project analyzed by boundary
    When I run "boundary diagram . --diagram-type layers"
    Then the output does not contain any node labeled "<file>"

  Scenario: Mermaid layer diagram does not contain synthetic package nodes
    Given a Go project analyzed by boundary
    When I run "boundary diagram . --diagram-type layers"
    Then the output does not contain any node labeled "<package>"

  Scenario: Mermaid output contains real component names
    Given a Go project with a UserRepository component in the infrastructure layer
    When I run "boundary diagram . --diagram-type layers"
    Then the output contains the component name "UserRepository"
    And the component is placed inside the "Infrastructure" subgraph

  Scenario: DOT output does not contain synthetic nodes
    Given a Go project analyzed by boundary
    When I run "boundary diagram . --diagram-type dot"
    Then the output does not contain any node labeled "<file>"
    And the output does not contain any node labeled "<package>"

  Scenario: Diagram command succeeds on a project with violations
    Given a project where a domain file imports from infrastructure (a layer boundary violation)
    When I run "boundary diagram . --diagram-type layers"
    Then the command exits with code 0
    And the output is a valid Mermaid flowchart
    And the output contains the expected architectural layer subgraphs
    And the output does not contain synthetic "<file>" nodes
    Note: File-level import edges (file->package) involve only synthetic nodes and are
          filtered out; named component-to-component violation edges are shown when present
