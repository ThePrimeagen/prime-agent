package db

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"regexp"
	"strings"

	"github.com/jmoiron/sqlx"
	_ "github.com/mattn/go-sqlite3"
)

type Store struct {
	db *sqlx.DB
}

type Skill struct {
	ID     int64  `db:"id"`
	Name   string `db:"name"`
	Prompt string `db:"prompt"`
}

type Pipeline struct {
	ID   int64  `db:"id"`
	Name string `db:"name"`
}

type PipelineStep struct {
	ID         int64  `db:"id"`
	PipelineID int64  `db:"pipeline_id"`
	Title      string `db:"title"`
	Prompt     string `db:"prompt"`
	Position   int64  `db:"position"`
	SkillCount int64  `db:"skill_count"`
	Skills     []PipelineStepSkill
}

type PipelineStepSkill struct {
	ID   int64  `db:"id"`
	Name string `db:"name"`
}

var ErrConflict = errors.New("conflict")

const nameRuleMessage = "name must contain only lowercase letters, digits, and dashes"

var kebabNamePattern = regexp.MustCompile(`^[a-z0-9-]+$`)

func NewStore(path string) (*Store, error) {
	db, err := sqlx.Open("sqlite3", path)
	if err != nil {
		return nil, fmt.Errorf("open sqlite database: %w", err)
	}

	if err := db.Ping(); err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("ping sqlite database: %w", err)
	}

	if _, err := db.Exec(`PRAGMA foreign_keys = ON;`); err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("enable sqlite foreign keys: %w", err)
	}

	if _, err := db.Exec(`
CREATE TABLE IF NOT EXISTS counter (
	id INTEGER PRIMARY KEY CHECK(id = 1),
	count INTEGER NOT NULL
);`); err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("create counter table: %w", err)
	}

	if _, err := db.Exec(`
INSERT INTO counter (id, count)
VALUES (1, 0)
ON CONFLICT(id) DO NOTHING;`); err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("seed counter row: %w", err)
	}

	if _, err := db.Exec(`
CREATE TABLE IF NOT EXISTS skills (
	id INTEGER PRIMARY KEY AUTOINCREMENT,
	name TEXT NOT NULL CHECK(length(trim(name)) > 0),
	prompt TEXT NOT NULL CHECK(length(trim(prompt)) > 0)
);`); err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("create skills table: %w", err)
	}

	if _, err := db.Exec(`
CREATE TABLE IF NOT EXISTS pipelines (
	id INTEGER PRIMARY KEY AUTOINCREMENT,
	name TEXT NOT NULL CHECK(length(trim(name)) > 0)
);`); err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("create pipelines table: %w", err)
	}

	if _, err := db.Exec(`
CREATE TABLE IF NOT EXISTS pipeline_steps (
	id INTEGER PRIMARY KEY AUTOINCREMENT,
	pipeline_id INTEGER NOT NULL,
	title TEXT NOT NULL CHECK(length(trim(title)) > 0),
	prompt TEXT NOT NULL CHECK(length(trim(prompt)) > 0),
	position INTEGER NOT NULL CHECK(position >= 0),
	UNIQUE (pipeline_id, position),
	FOREIGN KEY(pipeline_id) REFERENCES pipelines(id) ON DELETE CASCADE
);`); err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("create pipeline_steps table: %w", err)
	}

	if _, err := db.Exec(`
CREATE TABLE IF NOT EXISTS pipeline_step_skills (
	pipeline_step_id INTEGER NOT NULL,
	skill_id INTEGER NOT NULL,
	PRIMARY KEY (pipeline_step_id, skill_id),
	FOREIGN KEY(pipeline_step_id) REFERENCES pipeline_steps(id) ON DELETE CASCADE,
	FOREIGN KEY(skill_id) REFERENCES skills(id) ON DELETE CASCADE
);`); err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("create pipeline_step_skills table: %w", err)
	}

	return &Store{db: db}, nil
}

func (s *Store) Close() error {
	return s.db.Close()
}

func (s *Store) IncrementAndGet(ctx context.Context) (int64, error) {
	tx, err := s.db.BeginTxx(ctx, &sql.TxOptions{})
	if err != nil {
		return 0, fmt.Errorf("begin transaction: %w", err)
	}
	defer func() {
		if err != nil {
			_ = tx.Rollback()
		}
	}()

	if _, err = tx.ExecContext(ctx, `UPDATE counter SET count = count + 1 WHERE id = 1`); err != nil {
		return 0, fmt.Errorf("increment counter: %w", err)
	}

	var count int64
	if err = tx.GetContext(ctx, &count, `SELECT count FROM counter WHERE id = 1`); err != nil {
		return 0, fmt.Errorf("read counter: %w", err)
	}

	if err = tx.Commit(); err != nil {
		return 0, fmt.Errorf("commit transaction: %w", err)
	}

	return count, nil
}

func (s *Store) ListSkills(ctx context.Context) ([]Skill, error) {
	var skills []Skill
	if err := s.db.SelectContext(ctx, &skills, `SELECT id, name, prompt FROM skills ORDER BY id ASC`); err != nil {
		return nil, fmt.Errorf("list skills: %w", err)
	}
	return skills, nil
}

func (s *Store) GetSkill(ctx context.Context, id int64) (Skill, error) {
	var skill Skill
	if err := s.db.GetContext(ctx, &skill, `SELECT id, name, prompt FROM skills WHERE id = ?`, id); err != nil {
		if err == sql.ErrNoRows {
			return Skill{}, sql.ErrNoRows
		}
		return Skill{}, fmt.Errorf("get skill: %w", err)
	}
	return skill, nil
}

func (s *Store) CreateSkill(ctx context.Context, name, prompt string) (int64, error) {
	name = strings.TrimSpace(name)
	prompt = strings.TrimSpace(prompt)
	if !kebabNamePattern.MatchString(name) {
		return 0, fmt.Errorf("create skill: %s", nameRuleMessage)
	}
	if prompt == "" {
		return 0, fmt.Errorf("create skill: prompt is required")
	}

	result, err := s.db.ExecContext(ctx, `INSERT INTO skills (name, prompt) VALUES (?, ?)`, name, prompt)
	if err != nil {
		return 0, fmt.Errorf("create skill: %w", err)
	}
	id, err := result.LastInsertId()
	if err != nil {
		return 0, fmt.Errorf("read created skill id: %w", err)
	}
	return id, nil
}

func (s *Store) ListPipelines(ctx context.Context) ([]Pipeline, error) {
	var pipelines []Pipeline
	if err := s.db.SelectContext(ctx, &pipelines, `SELECT id, name FROM pipelines ORDER BY id ASC`); err != nil {
		return nil, fmt.Errorf("list pipelines: %w", err)
	}
	return pipelines, nil
}

func (s *Store) GetPipeline(ctx context.Context, id int64) (Pipeline, error) {
	var pipeline Pipeline
	if err := s.db.GetContext(ctx, &pipeline, `SELECT id, name FROM pipelines WHERE id = ?`, id); err != nil {
		if err == sql.ErrNoRows {
			return Pipeline{}, sql.ErrNoRows
		}
		return Pipeline{}, fmt.Errorf("get pipeline: %w", err)
	}
	return pipeline, nil
}

func (s *Store) CreatePipeline(ctx context.Context, name string) (int64, error) {
	name = strings.TrimSpace(name)
	if !kebabNamePattern.MatchString(name) {
		return 0, fmt.Errorf("create pipeline: %s", nameRuleMessage)
	}

	result, err := s.db.ExecContext(ctx, `INSERT INTO pipelines (name) VALUES (?)`, name)
	if err != nil {
		return 0, fmt.Errorf("create pipeline: %w", err)
	}
	id, err := result.LastInsertId()
	if err != nil {
		return 0, fmt.Errorf("read created pipeline id: %w", err)
	}
	return id, nil
}

func (s *Store) ListPipelineSteps(ctx context.Context, pipelineID int64) ([]PipelineStep, error) {
	type pipelineStepRow struct {
		ID         int64          `db:"id"`
		PipelineID int64          `db:"pipeline_id"`
		Title      string         `db:"title"`
		Prompt     string         `db:"prompt"`
		Position   int64          `db:"position"`
		SkillID    sql.NullInt64  `db:"skill_id"`
		SkillName  sql.NullString `db:"skill_name"`
	}

	var rows []pipelineStepRow
	if err := s.db.SelectContext(ctx, &rows, `
SELECT
	ps.id,
	ps.pipeline_id,
	ps.title,
	ps.prompt,
	ps.position,
	s.id AS skill_id,
	s.name AS skill_name
FROM pipeline_steps ps
LEFT JOIN pipeline_step_skills pss ON pss.pipeline_step_id = ps.id
LEFT JOIN skills s ON s.id = pss.skill_id
WHERE ps.pipeline_id = ?
ORDER BY ps.position ASC, s.name ASC`, pipelineID); err != nil {
		return nil, fmt.Errorf("list pipeline steps: %w", err)
	}

	steps := make([]PipelineStep, 0)
	stepIndexByID := make(map[int64]int, len(rows))
	for _, row := range rows {
		index, exists := stepIndexByID[row.ID]
		if !exists {
			steps = append(steps, PipelineStep{
				ID:         row.ID,
				PipelineID: row.PipelineID,
				Title:      row.Title,
				Prompt:     row.Prompt,
				Position:   row.Position,
			})
			index = len(steps) - 1
			stepIndexByID[row.ID] = index
		}
		if row.SkillID.Valid {
			steps[index].Skills = append(steps[index].Skills, PipelineStepSkill{
				ID:   row.SkillID.Int64,
				Name: row.SkillName.String,
			})
			steps[index].SkillCount++
		}
	}

	return steps, nil
}

func (s *Store) CreatePipelineStep(ctx context.Context, pipelineID int64, title, prompt string) (int64, error) {
	title = strings.TrimSpace(title)
	title = strings.ToLower(title)
	prompt = strings.TrimSpace(prompt)
	if title == "" {
		return 0, fmt.Errorf("create pipeline step: title is required")
	}
	if prompt == "" {
		return 0, fmt.Errorf("create pipeline step: prompt is required")
	}

	tx, err := s.db.BeginTxx(ctx, &sql.TxOptions{})
	if err != nil {
		return 0, fmt.Errorf("create pipeline step begin transaction: %w", err)
	}
	defer func() {
		if err != nil {
			_ = tx.Rollback()
		}
	}()

	var pipelineExists int64
	if err = tx.GetContext(ctx, &pipelineExists, `SELECT 1 FROM pipelines WHERE id = ?`, pipelineID); err != nil {
		if err == sql.ErrNoRows {
			return 0, sql.ErrNoRows
		}
		return 0, fmt.Errorf("create pipeline step verify pipeline: %w", err)
	}

	var nextPosition int64
	if err = tx.GetContext(ctx, &nextPosition, `SELECT COALESCE(MAX(position) + 1, 0) FROM pipeline_steps WHERE pipeline_id = ?`, pipelineID); err != nil {
		return 0, fmt.Errorf("create pipeline step compute position: %w", err)
	}

	result, err := tx.ExecContext(ctx, `
INSERT INTO pipeline_steps (pipeline_id, title, prompt, position)
VALUES (?, ?, ?, ?)`, pipelineID, title, prompt, nextPosition)
	if err != nil {
		return 0, fmt.Errorf("create pipeline step: %w", err)
	}

	id, err := result.LastInsertId()
	if err != nil {
		return 0, fmt.Errorf("create pipeline step read id: %w", err)
	}

	if err = tx.Commit(); err != nil {
		return 0, fmt.Errorf("create pipeline step commit transaction: %w", err)
	}
	return id, nil
}

func (s *Store) UpdatePipelineStep(ctx context.Context, pipelineID, stepID int64, title, prompt string) error {
	title = strings.TrimSpace(title)
	title = strings.ToLower(title)
	prompt = strings.TrimSpace(prompt)
	if title == "" {
		return fmt.Errorf("update pipeline step: title is required")
	}
	if prompt == "" {
		return fmt.Errorf("update pipeline step: prompt is required")
	}

	result, err := s.db.ExecContext(ctx, `
UPDATE pipeline_steps
SET title = ?, prompt = ?
WHERE id = ? AND pipeline_id = ?`, title, prompt, stepID, pipelineID)
	if err != nil {
		return fmt.Errorf("update pipeline step: %w", err)
	}
	rows, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("update pipeline step rows affected: %w", err)
	}
	if rows == 0 {
		return sql.ErrNoRows
	}
	return nil
}

func (s *Store) DeletePipelineStep(ctx context.Context, pipelineID, stepID int64) error {
	tx, err := s.db.BeginTxx(ctx, &sql.TxOptions{})
	if err != nil {
		return fmt.Errorf("delete pipeline step begin transaction: %w", err)
	}
	defer func() {
		if err != nil {
			_ = tx.Rollback()
		}
	}()

	var removedPosition int64
	if err = tx.GetContext(ctx, &removedPosition, `
SELECT position
FROM pipeline_steps
WHERE id = ? AND pipeline_id = ?`, stepID, pipelineID); err != nil {
		if err == sql.ErrNoRows {
			return sql.ErrNoRows
		}
		return fmt.Errorf("delete pipeline step lookup: %w", err)
	}

	result, err := tx.ExecContext(ctx, `DELETE FROM pipeline_steps WHERE id = ? AND pipeline_id = ?`, stepID, pipelineID)
	if err != nil {
		return fmt.Errorf("delete pipeline step: %w", err)
	}
	rows, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("delete pipeline step rows affected: %w", err)
	}
	if rows == 0 {
		return sql.ErrNoRows
	}

	if _, err = tx.ExecContext(ctx, `
UPDATE pipeline_steps
SET position = position - 1
WHERE pipeline_id = ? AND position > ?`, pipelineID, removedPosition); err != nil {
		return fmt.Errorf("delete pipeline step normalize positions: %w", err)
	}

	if err = tx.Commit(); err != nil {
		return fmt.Errorf("delete pipeline step commit transaction: %w", err)
	}
	return nil
}

func (s *Store) ReorderPipelineStep(ctx context.Context, pipelineID, stepID, targetStepID int64) error {
	tx, err := s.db.BeginTxx(ctx, &sql.TxOptions{})
	if err != nil {
		return fmt.Errorf("reorder pipeline step begin transaction: %w", err)
	}
	defer func() {
		if err != nil {
			_ = tx.Rollback()
		}
	}()

	var sourcePosition int64
	if err = tx.GetContext(ctx, &sourcePosition, `
SELECT position
FROM pipeline_steps
WHERE id = ? AND pipeline_id = ?`, stepID, pipelineID); err != nil {
		if err == sql.ErrNoRows {
			return sql.ErrNoRows
		}
		return fmt.Errorf("reorder pipeline step source lookup: %w", err)
	}

	var targetPosition int64
	if err = tx.GetContext(ctx, &targetPosition, `
SELECT position
FROM pipeline_steps
WHERE id = ? AND pipeline_id = ?`, targetStepID, pipelineID); err != nil {
		if err == sql.ErrNoRows {
			return sql.ErrNoRows
		}
		return fmt.Errorf("reorder pipeline step target lookup: %w", err)
	}

	if sourcePosition == targetPosition {
		if err = tx.Commit(); err != nil {
			return fmt.Errorf("reorder pipeline step commit transaction: %w", err)
		}
		return nil
	}

	var tempPosition int64
	if err = tx.GetContext(ctx, &tempPosition, `
SELECT COALESCE(MAX(position) + 1, 0)
FROM pipeline_steps
WHERE pipeline_id = ?`, pipelineID); err != nil {
		return fmt.Errorf("reorder pipeline step compute temp position: %w", err)
	}

	if _, err = tx.ExecContext(ctx, `
UPDATE pipeline_steps
SET position = ?
WHERE id = ? AND pipeline_id = ?`, tempPosition, stepID, pipelineID); err != nil {
		return fmt.Errorf("reorder pipeline step set source temp: %w", err)
	}
	if _, err = tx.ExecContext(ctx, `
UPDATE pipeline_steps
SET position = ?
WHERE id = ? AND pipeline_id = ?`, sourcePosition, targetStepID, pipelineID); err != nil {
		return fmt.Errorf("reorder pipeline step move target: %w", err)
	}
	if _, err = tx.ExecContext(ctx, `
UPDATE pipeline_steps
SET position = ?
WHERE id = ? AND pipeline_id = ?`, targetPosition, stepID, pipelineID); err != nil {
		return fmt.Errorf("reorder pipeline step place source: %w", err)
	}

	if err = tx.Commit(); err != nil {
		return fmt.Errorf("reorder pipeline step commit transaction: %w", err)
	}
	return nil
}

func (s *Store) AddPipelineStepSkill(ctx context.Context, pipelineID, stepID, skillID int64) error {
	tx, err := s.db.BeginTxx(ctx, &sql.TxOptions{})
	if err != nil {
		return fmt.Errorf("add pipeline step skill begin transaction: %w", err)
	}
	defer func() {
		if err != nil {
			_ = tx.Rollback()
		}
	}()

	var stepExists int64
	if err = tx.GetContext(ctx, &stepExists, `
SELECT 1 FROM pipeline_steps WHERE id = ? AND pipeline_id = ?`, stepID, pipelineID); err != nil {
		if err == sql.ErrNoRows {
			return sql.ErrNoRows
		}
		return fmt.Errorf("add pipeline step skill verify step: %w", err)
	}

	var skillExists int64
	if err = tx.GetContext(ctx, &skillExists, `SELECT 1 FROM skills WHERE id = ?`, skillID); err != nil {
		if err == sql.ErrNoRows {
			return sql.ErrNoRows
		}
		return fmt.Errorf("add pipeline step skill verify skill: %w", err)
	}

	if _, err = tx.ExecContext(ctx, `
INSERT INTO pipeline_step_skills (pipeline_step_id, skill_id)
VALUES (?, ?)`, stepID, skillID); err != nil {
		if strings.Contains(err.Error(), "UNIQUE constraint failed") {
			return fmt.Errorf("add pipeline step skill duplicate: %w", ErrConflict)
		}
		return fmt.Errorf("add pipeline step skill: %w", err)
	}

	if err = tx.Commit(); err != nil {
		return fmt.Errorf("add pipeline step skill commit transaction: %w", err)
	}
	return nil
}

func (s *Store) DeletePipelineStepSkill(ctx context.Context, pipelineID, stepID, skillID int64) error {
	var stepExists int64
	if err := s.db.GetContext(ctx, &stepExists, `
SELECT 1 FROM pipeline_steps WHERE id = ? AND pipeline_id = ?`, stepID, pipelineID); err != nil {
		if err == sql.ErrNoRows {
			return sql.ErrNoRows
		}
		return fmt.Errorf("delete pipeline step skill verify step: %w", err)
	}

	result, err := s.db.ExecContext(ctx, `
DELETE FROM pipeline_step_skills
WHERE pipeline_step_id = ? AND skill_id = ?`, stepID, skillID)
	if err != nil {
		return fmt.Errorf("delete pipeline step skill: %w", err)
	}
	rows, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("delete pipeline step skill rows affected: %w", err)
	}
	if rows == 0 {
		return sql.ErrNoRows
	}
	return nil
}

func (s *Store) UpdateSkill(ctx context.Context, id int64, name, prompt string) error {
	name = strings.TrimSpace(name)
	prompt = strings.TrimSpace(prompt)
	if !kebabNamePattern.MatchString(name) {
		return fmt.Errorf("update skill: %s", nameRuleMessage)
	}
	if prompt == "" {
		return fmt.Errorf("update skill: prompt is required")
	}

	result, err := s.db.ExecContext(ctx, `UPDATE skills SET name = ?, prompt = ? WHERE id = ?`, name, prompt, id)
	if err != nil {
		return fmt.Errorf("update skill: %w", err)
	}
	rows, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("update skill rows affected: %w", err)
	}
	if rows == 0 {
		return sql.ErrNoRows
	}
	return nil
}

func (s *Store) DeleteSkill(ctx context.Context, id int64) error {
	result, err := s.db.ExecContext(ctx, `DELETE FROM skills WHERE id = ?`, id)
	if err != nil {
		return fmt.Errorf("delete skill: %w", err)
	}
	rows, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("delete skill rows affected: %w", err)
	}
	if rows == 0 {
		return sql.ErrNoRows
	}
	return nil
}
