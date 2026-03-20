package web

import (
	"context"
	"database/sql"
	"errors"
	"html/template"
	"log"
	"net/http"
	"strconv"
	"strings"

	"prime-agent/internal/db"
	"prime-agent/web/templates"
)

type AppStore interface {
	IncrementAndGet(ctx context.Context) (int64, error)
	ListSkills(ctx context.Context) ([]db.Skill, error)
	GetSkill(ctx context.Context, id int64) (db.Skill, error)
	CreateSkill(ctx context.Context, name, prompt string) (int64, error)
	UpdateSkill(ctx context.Context, id int64, name, prompt string) error
	DeleteSkill(ctx context.Context, id int64) error
	ListPipelines(ctx context.Context) ([]db.Pipeline, error)
	GetPipeline(ctx context.Context, id int64) (db.Pipeline, error)
	CreatePipeline(ctx context.Context, name string) (int64, error)
	ListPipelineSteps(ctx context.Context, pipelineID int64) ([]db.PipelineStep, error)
	CreatePipelineStep(ctx context.Context, pipelineID int64, title, prompt string) (int64, error)
	UpdatePipelineStep(ctx context.Context, pipelineID, stepID int64, title, prompt string) error
	DeletePipelineStep(ctx context.Context, pipelineID, stepID int64) error
	ReorderPipelineStep(ctx context.Context, pipelineID, stepID, targetStepID int64) error
	AddPipelineStepSkill(ctx context.Context, pipelineID, stepID, skillID int64) error
	DeletePipelineStepSkill(ctx context.Context, pipelineID, stepID, skillID int64) error
}

type pageData struct {
	ActiveSection    string
	Skills           []db.Skill
	SelectedSkill    *db.Skill
	Pipelines        []db.Pipeline
	SelectedPipeline *db.Pipeline
	PipelineSteps    []db.PipelineStep
}

var pageTmpl = template.Must(template.New("page").Funcs(template.FuncMap{
	"joinStepSkillNames": joinStepSkillNames,
}).Parse(`<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Hello World</title>
    <script src="https://unpkg.com/htmx.org@2.0.4"></script>
  </head>
  <body style="margin:0;font-family:system-ui, sans-serif;">
    <div id="app-shell" style="display:flex;min-height:100vh;">
      <nav id="left-nav" style="width:20%;min-width:220px;border-right:1px solid #ddd;padding:1rem;box-sizing:border-box;">
        <div style="display:flex;gap:0.5rem;margin-bottom:1rem;">
          <a
            id="tab-skills"
            data-testid="tab-skills"
            data-icon="sword"
            title="Skills"
            aria-label="Skills"
            href="/skills"
            aria-current="{{if eq .ActiveSection "skills"}}page{{else}}false{{end}}"
            style="display:inline-flex;align-items:center;justify-content:center;width:2rem;height:2rem;border:1px solid #bbb;border-radius:6px;background:#fff;cursor:pointer;text-decoration:none;color:inherit;">&#9876;</a>
          <a
            id="tab-pipeline"
            data-testid="tab-pipeline"
            data-icon="pipe"
            title="Pipeline"
            aria-label="Pipeline"
            href="/pipelines"
            aria-current="{{if eq .ActiveSection "pipelines"}}page{{else}}false{{end}}"
            style="display:inline-flex;align-items:center;justify-content:center;width:2rem;height:2rem;border:1px solid #bbb;border-radius:6px;background:#fff;cursor:pointer;text-decoration:none;color:inherit;">&#x2554;&#x2557;</a>
        </div>

        <section id="skills-nav-panel" data-tab-panel="skills" {{if ne .ActiveSection "skills"}}hidden{{end}}>
          <h2 style="margin-top:0;">Skills</h2>
          <button
            type="button"
            data-testid="skill-create-open"
            style="background:#2563eb;color:#fff;border:none;border-radius:6px;padding:0.35rem 0.8rem;cursor:pointer;"
            onclick="document.getElementById('skill-modal').showModal();">[ + ]</button>

          <dialog id="skill-modal">
            <form method="post" action="/skills">
              <h3>Create Skill</h3>
              <label>Name<br><input name="name" required></label><br><br>
              <label>Prompt<br><textarea name="prompt" rows="6" cols="40" required></textarea></label><br><br>
              <button type="submit">Save</button>
              <button type="button" onclick="document.getElementById('skill-modal').close();">Cancel</button>
            </form>
          </dialog>

          {{if .Skills}}
          <ul id="skills-nav-list" style="padding-left:0;margin-top:1rem;">
            {{range .Skills}}
            <li style="list-style:none;margin-top:0.5rem;">
              <a
                data-testid="skill-nav-link"
                data-selected="{{if and $.SelectedSkill (eq $.SelectedSkill.ID .ID)}}true{{else}}false{{end}}"
                href="/skills/{{.ID}}"
                style="display:block;padding:0.4rem 0.5rem;border:1px solid #ddd;border-radius:6px;text-decoration:none;color:inherit;">{{.Name}}</a>
            </li>
            {{end}}
          </ul>
          {{else}}
          <p>No skills yet.</p>
          {{end}}
        </section>

        <section id="pipeline-nav-panel" data-tab-panel="pipeline" {{if ne .ActiveSection "pipelines"}}hidden{{end}}>
          <h2 style="margin-top:0;">Pipeline</h2>
          <button
            type="button"
            data-testid="pipeline-create-open"
            style="background:#2563eb;color:#fff;border:none;border-radius:6px;padding:0.35rem 0.8rem;cursor:pointer;"
            onclick="document.getElementById('pipeline-modal').showModal();">[ + ]</button>

          <dialog id="pipeline-modal">
            <form method="post" action="/pipelines">
              <h3>Create Pipeline</h3>
              <label>Name<br><input name="name" data-lowercase-field="pipeline-name" required></label><br><br>
              <button type="submit">Save</button>
              <button type="button" onclick="document.getElementById('pipeline-modal').close();">Cancel</button>
            </form>
          </dialog>

          {{if .Pipelines}}
          <ul id="pipeline-nav-list" style="padding-left:0;margin-top:1rem;">
            {{range .Pipelines}}
            <li style="list-style:none;margin-top:0.5rem;">
              <a
                data-testid="pipeline-nav-link"
                data-selected="{{if and $.SelectedPipeline (eq $.SelectedPipeline.ID .ID)}}true{{else}}false{{end}}"
                href="/pipelines/{{.ID}}"
                style="display:block;padding:0.4rem 0.5rem;border:1px solid #ddd;border-radius:6px;text-decoration:none;color:inherit;">{{.Name}}</a>
            </li>
            {{end}}
          </ul>
          {{else}}
          <p>No pipelines yet.</p>
          {{end}}

          {{if .SelectedPipeline}}
          <h3 style="margin-top:1.25rem;margin-bottom:0.25rem;">Steps</h3>
          {{if .PipelineSteps}}
          <ul id="pipeline-step-nav-list" style="padding-left:0;margin-top:0.5rem;">
            {{range .PipelineSteps}}
            <li
              data-testid="pipeline-step-nav-item"
              data-step-id="{{.ID}}"
              data-reorder-endpoint="/pipelines/{{$.SelectedPipeline.ID}}/steps/{{.ID}}/reorder"
              draggable="true"
              style="list-style:none;margin-top:0.4rem;padding:0.45rem 0.55rem;border:1px solid #ddd;border-radius:6px;background:#fff;display:flex;justify-content:space-between;align-items:center;gap:0.5rem;cursor:grab;">
              <span>{{.Title}}</span>
              <span style="display:flex;flex-direction:column;align-items:flex-end;gap:0.1rem;">
                <span data-testid="pipeline-step-skill-count" style="font-size:0.8rem;color:#4b5563;">{{.SkillCount}}</span>
                <span data-testid="pipeline-step-skill-summary" style="font-size:0.72rem;color:#6b7280;">{{if .Skills}}{{joinStepSkillNames .Skills}}{{else}}No skills{{end}}</span>
              </span>
            </li>
            {{end}}
          </ul>
          {{else}}
          <p style="margin-top:0.5rem;">No steps yet.</p>
          {{end}}
          {{end}}
        </section>
      </nav>
      <main id="main-content" style="width:80%;padding:1rem;box-sizing:border-box;">
        <section id="skills-main-panel" data-tab-main="skills" {{if ne .ActiveSection "skills"}}hidden{{end}}>
          <h1>Skills</h1>
          {{if .SelectedSkill}}
          <article data-skill-editor data-skill-id="{{.SelectedSkill.ID}}" style="max-width:740px;border:1px solid #ddd;padding:0.8rem;margin-top:0.8rem;position:relative;">
            <form method="post" action="/skills/{{.SelectedSkill.ID}}/update" data-skill-form>
              <label>Name<br><input name="name" value="{{.SelectedSkill.Name}}" required></label><br><br>
              <label>Prompt<br><textarea name="prompt" rows="8" cols="70" required>{{.SelectedSkill.Prompt}}</textarea></label><br><br>
              <div style="position:absolute;top:0.5rem;right:0.6rem;display:flex;align-items:center;gap:0.35rem;">
                <span
                  data-testid="autosave-status"
                  data-save-state="saved"
                  aria-live="polite"
                  aria-label="Saved"
                  title="Saved"
                  style="font-size:0.85rem;line-height:1;color:#047857;">✓</span>
                <div data-delete-controls style="position:relative;">
                  <button
                    type="button"
                    data-testid="delete-skill-trigger"
                    aria-label="Delete skill"
                    aria-haspopup="dialog"
                    aria-expanded="false"
                    title="Delete skill"
                    style="border:none;background:transparent;color:#dc2626;cursor:pointer;font-size:0.85rem;line-height:1;padding:0;">&#128465;</button>
                  <div
                    data-testid="delete-skill-popover"
                    data-delete-popover
                    hidden
                    style="position:absolute;top:1.1rem;right:0;background:#fff;border:1px solid #d1d5db;border-radius:6px;padding:0.25rem;box-shadow:0 2px 8px rgba(0,0,0,0.12);z-index:10;white-space:nowrap;">
                    <button
                      type="button"
                      data-delete-confirm
                      style="border:1px solid #ef4444;background:#fff;color:#b91c1c;border-radius:4px;padding:0.1rem 0.35rem;font-size:0.72rem;cursor:pointer;">are you sure</button>
                  </div>
                </div>
              </div>
            </form>
            <form method="post" action="/skills/{{.SelectedSkill.ID}}/delete" data-skill-delete-form></form>
          </article>
          {{else}}
            {{if .Skills}}
            <p>Select a skill from the left nav.</p>
            {{else}}
            <p>No skills yet.</p>
            {{end}}
          {{end}}
        </section>

        <section id="pipeline-main-panel" data-tab-main="pipeline" {{if ne .ActiveSection "pipelines"}}hidden{{end}}>
          {{if .SelectedPipeline}}
          <h1 id="pipeline-title">{{.SelectedPipeline.Name}}</h1>
          <button
            type="button"
            data-testid="pipeline-step-create-open"
            style="background:#2563eb;color:#fff;border:none;border-radius:6px;padding:0.35rem 0.8rem;cursor:pointer;"
            onclick="document.getElementById('pipeline-step-modal').showModal();">Add Step</button>

          <dialog id="pipeline-step-modal">
            <form method="post" action="/pipelines/{{.SelectedPipeline.ID}}/steps">
              <h3>Create Step</h3>
              <label>Title<br><input name="title" data-lowercase-field="pipeline-step-title" required></label><br><br>
              <label>Prompt<br><textarea name="prompt" rows="6" cols="40" required></textarea></label><br><br>
              <button type="submit">Save</button>
              <button type="button" onclick="document.getElementById('pipeline-step-modal').close();">Cancel</button>
            </form>
          </dialog>

          {{if .PipelineSteps}}
            {{range .PipelineSteps}}
            <article data-testid="pipeline-step-editor" style="max-width:740px;border:1px solid #ddd;padding:0.8rem;margin-top:0.8rem;position:relative;">
              <form method="post" action="/pipelines/{{$.SelectedPipeline.ID}}/steps/{{.ID}}/update">
                <label>Title<br><input name="title" data-lowercase-field="pipeline-step-title" value="{{.Title}}" required></label><br><br>
                <label>Description<br><textarea name="prompt" rows="6" cols="70" required>{{.Prompt}}</textarea></label><br><br>
                <button type="submit" data-testid="pipeline-step-save">Save</button>
              </form>
              <div style="display:flex;gap:0.5rem;align-items:center;flex-wrap:wrap;">
                <form method="post" action="/pipelines/{{$.SelectedPipeline.ID}}/steps/{{.ID}}/skills" style="display:flex;gap:0.4rem;align-items:center;">
                  <select name="skill_id" required>
                    <option value="">Select skill</option>
                    {{range $.Skills}}
                    <option value="{{.ID}}">{{.Name}}</option>
                    {{end}}
                  </select>
                  <button type="submit">Add Skill</button>
                </form>
                <form method="post" action="/pipelines/{{$.SelectedPipeline.ID}}/steps/{{.ID}}/delete">
                  <button type="submit" data-testid="pipeline-step-delete" style="border:1px solid #ef4444;background:#fff;color:#b91c1c;border-radius:4px;padding:0.2rem 0.45rem;font-size:0.8rem;cursor:pointer;">Delete</button>
                </form>
              </div>
              {{$step := .}}
              {{if .Skills}}
              <ul style="padding-left:1.1rem;margin:0.6rem 0 0 0;">
                {{range .Skills}}
                <li style="margin-top:0.3rem;">
                  <span data-testid="pipeline-step-attached-skill">{{.Name}}</span>
                  <form method="post" action="/pipelines/{{$.SelectedPipeline.ID}}/steps/{{$step.ID}}/skills/{{.ID}}/delete" style="display:inline;">
                    <button type="submit" style="margin-left:0.4rem;border:1px solid #ef4444;background:#fff;color:#b91c1c;border-radius:4px;padding:0.1rem 0.35rem;font-size:0.72rem;cursor:pointer;">Remove</button>
                  </form>
                </li>
                {{end}}
              </ul>
              {{else}}
              <p style="margin-top:0.6rem;color:#6b7280;">No skills attached.</p>
              {{end}}
            </article>
            {{end}}
          {{else}}
            <p>No steps yet.</p>
          {{end}}
          {{else}}
          <h1>Pipeline</h1>
          {{if .Pipelines}}
          <p>Select a pipeline from the left nav.</p>
          {{else}}
          <p>No pipelines yet.</p>
          {{end}}
          {{end}}
        </section>
      </main>
    </div>
    <script>
      const autosaveState = new Map();

      function getEditorState(editor) {
        const id = editor.getAttribute("data-skill-id") || "";
        if (!autosaveState.has(id)) {
          autosaveState.set(id, { inFlight: false, pending: false, retryTimer: null });
        }
        return autosaveState.get(id);
      }

      function setSaveStatus(editor, state, color) {
        const status = editor.querySelector("[data-testid='autosave-status']");
        if (!status) {
          return;
        }
        let label = "Saved";
        if (state === "saving") {
          label = "Saving";
        } else if (state === "retrying") {
          label = "Retrying";
        }
        status.textContent = "✓";
        status.setAttribute("data-save-state", state);
        status.setAttribute("aria-label", label);
        status.setAttribute("title", label);
        status.style.color = color;
      }

      function scheduleRetry(editor) {
        const state = getEditorState(editor);
        if (state.retryTimer) {
          clearTimeout(state.retryTimer);
        }
        state.retryTimer = window.setTimeout(() => {
          saveEditor(editor);
        }, 1000);
      }

      async function saveEditor(editor) {
        const state = getEditorState(editor);
        if (state.inFlight) {
          state.pending = true;
          return;
        }

        const form = editor.querySelector("[data-skill-form]");
        if (!form) {
          return;
        }

        const nameInput = form.querySelector("input[name='name']");
        const promptInput = form.querySelector("textarea[name='prompt']");
        if (!nameInput || !promptInput) {
          return;
        }

        state.pending = false;
        state.inFlight = true;
        setSaveStatus(editor, "saving", "#6b7280");
        const payload = new URLSearchParams();
        payload.set("name", nameInput.value);
        payload.set("prompt", promptInput.value);

        try {
          const response = await fetch(form.action, {
            method: "POST",
            headers: {
              "Content-Type": "application/x-www-form-urlencoded;charset=UTF-8",
              "X-Autosave": "1"
            },
            body: payload.toString()
          });
          if (!response.ok) {
            throw new Error("autosave failed");
          }
          if (state.retryTimer) {
            clearTimeout(state.retryTimer);
            state.retryTimer = null;
          }
          setSaveStatus(editor, "saved", "#047857");
        } catch (_error) {
          setSaveStatus(editor, "retrying", "#dc2626");
          scheduleRetry(editor);
        } finally {
          state.inFlight = false;
          if (state.pending) {
            saveEditor(editor);
          }
        }
      }

      function wireAutosave() {
        const editors = document.querySelectorAll("[data-skill-editor]");
        for (const editor of editors) {
          const form = editor.querySelector("[data-skill-form]");
          const fields = form?.querySelectorAll("input[name='name'], textarea[name='prompt']");
          if (!fields) {
            continue;
          }
          for (const field of fields) {
            field.addEventListener("input", () => {
              const state = getEditorState(editor);
              state.pending = true;
              if (state.retryTimer) {
                clearTimeout(state.retryTimer);
                state.retryTimer = null;
              }
              saveEditor(editor);
            });
          }
        }
      }

      function closeDeletePopovers(exceptControl) {
        const controls = document.querySelectorAll("[data-delete-controls]");
        for (const control of controls) {
          if (exceptControl && control === exceptControl) {
            continue;
          }
          const trigger = control.querySelector("[data-testid='delete-skill-trigger']");
          const popover = control.querySelector("[data-delete-popover]");
          if (trigger) {
            trigger.setAttribute("aria-expanded", "false");
          }
          if (popover) {
            popover.hidden = true;
          }
        }
      }

      function wireDeleteConfirmations() {
        const editors = document.querySelectorAll("[data-skill-editor]");
        for (const editor of editors) {
          const control = editor.querySelector("[data-delete-controls]");
          const trigger = editor.querySelector("[data-testid='delete-skill-trigger']");
          const popover = editor.querySelector("[data-delete-popover]");
          const confirm = editor.querySelector("[data-delete-confirm]");
          const form = editor.querySelector("[data-skill-delete-form]");
          if (!control || !trigger || !popover || !confirm || !form) {
            continue;
          }

          trigger.addEventListener("click", (event) => {
            event.stopPropagation();
            const open = popover.hidden;
            closeDeletePopovers(control);
            popover.hidden = !open;
            trigger.setAttribute("aria-expanded", open ? "true" : "false");
          });

          popover.addEventListener("click", (event) => {
            event.stopPropagation();
          });

          confirm.addEventListener("click", (event) => {
            event.stopPropagation();
            closeDeletePopovers();
            if (form.requestSubmit) {
              form.requestSubmit();
            } else {
              form.submit();
            }
          });
        }

        document.addEventListener("click", (event) => {
          for (const control of document.querySelectorAll("[data-delete-controls]")) {
            if (control.contains(event.target)) {
              return;
            }
          }
          closeDeletePopovers();
        });
      }

      function wirePipelineStepReorder() {
        const list = document.getElementById("pipeline-step-nav-list");
        if (!list) {
          return;
        }
        let dragging = null;
        const rows = list.querySelectorAll("[data-testid='pipeline-step-nav-item']");
        for (const row of rows) {
          row.addEventListener("dragstart", () => {
            dragging = row;
            row.style.opacity = "0.6";
          });
          row.addEventListener("dragend", () => {
            row.style.opacity = "1";
            dragging = null;
          });
          row.addEventListener("dragover", (event) => {
            event.preventDefault();
          });
          row.addEventListener("drop", async (event) => {
            event.preventDefault();
            if (!dragging || dragging === row) {
              return;
            }
            const draggingID = dragging.getAttribute("data-step-id");
            const targetID = row.getAttribute("data-step-id");
            const endpoint = dragging.getAttribute("data-reorder-endpoint");
            if (!draggingID || !targetID || !endpoint) {
              return;
            }

            list.insertBefore(dragging, row);
            const payload = new URLSearchParams();
            payload.set("target_step_id", targetID);
            try {
              const response = await fetch(endpoint, {
                method: "POST",
                headers: {
                  "Content-Type": "application/x-www-form-urlencoded;charset=UTF-8"
                },
                body: payload.toString()
              });
              if (!response.ok) {
                throw new Error("reorder failed");
              }
            } catch (_error) {
              window.location.reload();
            }
          });
        }
      }

      function normalizeLowercaseField(field) {
        const nextValue = field.value.toLowerCase();
        if (field.value !== nextValue) {
          field.value = nextValue;
        }
      }

      function wireLowercaseInputs() {
        const lowercaseFields = document.querySelectorAll(
          "input[data-lowercase-field='pipeline-name'], input[data-lowercase-field='pipeline-step-title']"
        );
        for (const field of lowercaseFields) {
          normalizeLowercaseField(field);
          field.addEventListener("input", () => {
            normalizeLowercaseField(field);
          });
        }
      }

      wireLowercaseInputs();
      wireAutosave();
      wireDeleteConfirmations();
      wirePipelineStepReorder();
    </script>
  </body>
</html>`))

func NewMux(store AppStore) http.Handler {
	mux := http.NewServeMux()
	mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/" {
			http.NotFound(w, r)
			return
		}

		if r.Method != http.MethodGet {
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}
		renderPage(w, r, store, "pipelines", nil, nil)
	})

	mux.HandleFunc("/fragments/counter", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodGet {
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}

		count, err := store.IncrementAndGet(r.Context())
		if err != nil {
			log.Printf("increment counter for fragment failed: %v", err)
			http.Error(w, "internal server error", http.StatusInternalServerError)
			return
		}

		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		if err := templates.Counter(count).Render(r.Context(), w); err != nil {
			log.Printf("render counter fragment failed: %v", err)
			http.Error(w, "internal server error", http.StatusInternalServerError)
			return
		}
	})

	mux.HandleFunc("/skills", func(w http.ResponseWriter, r *http.Request) {
		switch r.Method {
		case http.MethodGet:
			renderPage(w, r, store, "skills", nil, nil)
			return
		case http.MethodPost:
		default:
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}
		if err := r.ParseForm(); err != nil {
			http.Error(w, "invalid form", http.StatusBadRequest)
			return
		}
		name := strings.TrimSpace(r.FormValue("name"))
		prompt := strings.TrimSpace(r.FormValue("prompt"))
		if prompt == "" {
			http.Error(w, "prompt is required", http.StatusBadRequest)
			return
		}
		id, err := store.CreateSkill(r.Context(), name, prompt)
		if err != nil {
			if isNameValidationError(err) {
				http.Error(w, dbNameValidationMessage, http.StatusBadRequest)
				return
			}
			log.Printf("create skill failed: %v", err)
			http.Error(w, "internal server error", http.StatusInternalServerError)
			return
		}
		http.Redirect(w, r, "/skills/"+strconv.FormatInt(id, 10), http.StatusSeeOther)
	})

	mux.HandleFunc("/pipelines", func(w http.ResponseWriter, r *http.Request) {
		switch r.Method {
		case http.MethodGet:
			renderPage(w, r, store, "pipelines", nil, nil)
			return
		case http.MethodPost:
		default:
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}
		if err := r.ParseForm(); err != nil {
			http.Error(w, "invalid form", http.StatusBadRequest)
			return
		}
		name := strings.TrimSpace(r.FormValue("name"))
		id, err := store.CreatePipeline(r.Context(), name)
		if err != nil {
			if isNameValidationError(err) {
				http.Error(w, dbNameValidationMessage, http.StatusBadRequest)
				return
			}
			log.Printf("create pipeline failed: %v", err)
			http.Error(w, "internal server error", http.StatusInternalServerError)
			return
		}
		http.Redirect(w, r, "/pipelines/"+strconv.FormatInt(id, 10), http.StatusSeeOther)
	})

	mux.HandleFunc("/skills/", func(w http.ResponseWriter, r *http.Request) {
		if r.Method == http.MethodGet {
			skillID, ok := parseIDPath("/skills/", r.URL.Path)
			if !ok {
				http.NotFound(w, r)
				return
			}
			renderPage(w, r, store, "skills", &skillID, nil)
			return
		}
		if r.Method != http.MethodPost {
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}
		autosave := r.Header.Get("X-Autosave") == "1"

		id, action, ok := parseSkillPath(r.URL.Path)
		if !ok {
			http.Error(w, "invalid skill route", http.StatusBadRequest)
			return
		}
		if autosave && action != "update" {
			http.Error(w, "invalid autosave action", http.StatusBadRequest)
			return
		}

		switch action {
		case "update":
			if err := r.ParseForm(); err != nil {
				http.Error(w, "invalid form", http.StatusBadRequest)
				return
			}
			name := strings.TrimSpace(r.FormValue("name"))
			prompt := strings.TrimSpace(r.FormValue("prompt"))
			if prompt == "" {
				http.Error(w, "prompt is required", http.StatusBadRequest)
				return
			}
			if err := store.UpdateSkill(r.Context(), id, name, prompt); err != nil {
				if err == sql.ErrNoRows {
					http.NotFound(w, r)
					return
				}
				if isNameValidationError(err) {
					http.Error(w, dbNameValidationMessage, http.StatusBadRequest)
					return
				}
				log.Printf("update skill failed: %v", err)
				http.Error(w, "internal server error", http.StatusInternalServerError)
				return
			}
			if autosave {
				w.WriteHeader(http.StatusNoContent)
				return
			}
			http.Redirect(w, r, "/skills/"+strconv.FormatInt(id, 10), http.StatusSeeOther)
			return
		case "delete":
			if err := store.DeleteSkill(r.Context(), id); err != nil {
				if err == sql.ErrNoRows {
					http.NotFound(w, r)
					return
				}
				log.Printf("delete skill failed: %v", err)
				http.Error(w, "internal server error", http.StatusInternalServerError)
				return
			}
		default:
			http.NotFound(w, r)
			return
		}
		http.Redirect(w, r, "/skills", http.StatusSeeOther)
	})

	mux.HandleFunc("/pipelines/", func(w http.ResponseWriter, r *http.Request) {
		if r.Method == http.MethodGet {
			pipelineID, ok := parseIDPath("/pipelines/", r.URL.Path)
			if !ok {
				var rest []string
				pipelineID, rest, ok = parsePipelinePath(r.URL.Path)
				if !ok || len(rest) != 2 || rest[0] != "steps" {
					http.NotFound(w, r)
					return
				}
				if _, err := strconv.ParseInt(rest[1], 10, 64); err != nil {
					http.NotFound(w, r)
					return
				}
			}
			renderPage(w, r, store, "pipelines", nil, &pipelineID)
			return
		}
		if r.Method != http.MethodPost {
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}
		pipelineID, rest, ok := parsePipelinePath(r.URL.Path)
		if !ok {
			http.Error(w, "invalid pipeline route", http.StatusBadRequest)
			return
		}

		if len(rest) == 1 && rest[0] == "steps" {
			if err := r.ParseForm(); err != nil {
				http.Error(w, "invalid form", http.StatusBadRequest)
				return
			}
			title := strings.TrimSpace(r.FormValue("title"))
			prompt := strings.TrimSpace(r.FormValue("prompt"))
			if title == "" || prompt == "" {
				http.Error(w, "title and prompt are required", http.StatusBadRequest)
				return
			}
			stepID, err := store.CreatePipelineStep(r.Context(), pipelineID, title, prompt)
			if err != nil {
				if err == sql.ErrNoRows {
					http.NotFound(w, r)
					return
				}
				log.Printf("create pipeline step failed: %v", err)
				http.Error(w, "internal server error", http.StatusInternalServerError)
				return
			}
			http.Redirect(w, r, "/pipelines/"+strconv.FormatInt(pipelineID, 10)+"/steps/"+strconv.FormatInt(stepID, 10), http.StatusSeeOther)
			return
		}

		if len(rest) == 3 && rest[0] == "steps" && rest[2] == "delete" {
			stepID, err := strconv.ParseInt(rest[1], 10, 64)
			if err != nil {
				http.Error(w, "invalid step id", http.StatusBadRequest)
				return
			}
			if err := store.DeletePipelineStep(r.Context(), pipelineID, stepID); err != nil {
				if err == sql.ErrNoRows {
					http.NotFound(w, r)
					return
				}
				log.Printf("delete pipeline step failed: %v", err)
				http.Error(w, "internal server error", http.StatusInternalServerError)
				return
			}
			http.Redirect(w, r, "/pipelines/"+strconv.FormatInt(pipelineID, 10), http.StatusSeeOther)
			return
		}

		if len(rest) == 3 && rest[0] == "steps" && rest[2] == "update" {
			stepID, err := strconv.ParseInt(rest[1], 10, 64)
			if err != nil {
				http.Error(w, "invalid step id", http.StatusBadRequest)
				return
			}
			if err := r.ParseForm(); err != nil {
				http.Error(w, "invalid form", http.StatusBadRequest)
				return
			}
			title := strings.TrimSpace(r.FormValue("title"))
			prompt := strings.TrimSpace(r.FormValue("prompt"))
			if title == "" || prompt == "" {
				http.Error(w, "title and prompt are required", http.StatusBadRequest)
				return
			}
			if err := store.UpdatePipelineStep(r.Context(), pipelineID, stepID, title, prompt); err != nil {
				if err == sql.ErrNoRows {
					http.NotFound(w, r)
					return
				}
				log.Printf("update pipeline step failed: %v", err)
				http.Error(w, "internal server error", http.StatusInternalServerError)
				return
			}
			http.Redirect(w, r, "/pipelines/"+strconv.FormatInt(pipelineID, 10), http.StatusSeeOther)
			return
		}

		if len(rest) == 3 && rest[0] == "steps" && rest[2] == "reorder" {
			stepID, err := strconv.ParseInt(rest[1], 10, 64)
			if err != nil {
				http.Error(w, "invalid step id", http.StatusBadRequest)
				return
			}
			if err := r.ParseForm(); err != nil {
				http.Error(w, "invalid form", http.StatusBadRequest)
				return
			}
			targetID, err := strconv.ParseInt(strings.TrimSpace(r.FormValue("target_step_id")), 10, 64)
			if err != nil {
				http.Error(w, "target_step_id is required", http.StatusBadRequest)
				return
			}
			if err := store.ReorderPipelineStep(r.Context(), pipelineID, stepID, targetID); err != nil {
				if err == sql.ErrNoRows {
					http.NotFound(w, r)
					return
				}
				log.Printf("reorder pipeline step failed: %v", err)
				http.Error(w, "internal server error", http.StatusInternalServerError)
				return
			}
			http.Redirect(w, r, "/pipelines/"+strconv.FormatInt(pipelineID, 10), http.StatusSeeOther)
			return
		}

		if len(rest) == 3 && rest[0] == "steps" && rest[2] == "skills" {
			stepID, err := strconv.ParseInt(rest[1], 10, 64)
			if err != nil {
				http.Error(w, "invalid step id", http.StatusBadRequest)
				return
			}
			if err := r.ParseForm(); err != nil {
				http.Error(w, "invalid form", http.StatusBadRequest)
				return
			}
			skillID, err := strconv.ParseInt(strings.TrimSpace(r.FormValue("skill_id")), 10, 64)
			if err != nil {
				http.Error(w, "skill_id is required", http.StatusBadRequest)
				return
			}
			if err := store.AddPipelineStepSkill(r.Context(), pipelineID, stepID, skillID); err != nil {
				if err == sql.ErrNoRows {
					http.NotFound(w, r)
					return
				}
				if errors.Is(err, db.ErrConflict) {
					http.Error(w, "skill already attached to step", http.StatusBadRequest)
					return
				}
				log.Printf("add pipeline step skill failed: %v", err)
				http.Error(w, "internal server error", http.StatusInternalServerError)
				return
			}
			http.Redirect(w, r, "/pipelines/"+strconv.FormatInt(pipelineID, 10), http.StatusSeeOther)
			return
		}

		if len(rest) == 5 && rest[0] == "steps" && rest[2] == "skills" && rest[4] == "delete" {
			stepID, err := strconv.ParseInt(rest[1], 10, 64)
			if err != nil {
				http.Error(w, "invalid step id", http.StatusBadRequest)
				return
			}
			skillID, err := strconv.ParseInt(rest[3], 10, 64)
			if err != nil {
				http.Error(w, "invalid skill id", http.StatusBadRequest)
				return
			}
			if err := store.DeletePipelineStepSkill(r.Context(), pipelineID, stepID, skillID); err != nil {
				if err == sql.ErrNoRows {
					http.NotFound(w, r)
					return
				}
				log.Printf("delete pipeline step skill failed: %v", err)
				http.Error(w, "internal server error", http.StatusInternalServerError)
				return
			}
			http.Redirect(w, r, "/pipelines/"+strconv.FormatInt(pipelineID, 10), http.StatusSeeOther)
			return
		}

		http.NotFound(w, r)
	})

	return mux
}

func renderPage(w http.ResponseWriter, r *http.Request, store AppStore, activeSection string, selectedSkillID, selectedPipelineID *int64) {
	skills, err := store.ListSkills(r.Context())
	if err != nil {
		log.Printf("list skills for %s failed: %v", r.URL.Path, err)
		http.Error(w, "internal server error", http.StatusInternalServerError)
		return
	}

	pipelines, err := store.ListPipelines(r.Context())
	if err != nil {
		log.Printf("list pipelines for %s failed: %v", r.URL.Path, err)
		http.Error(w, "internal server error", http.StatusInternalServerError)
		return
	}

	var selectedSkill *db.Skill
	if selectedSkillID != nil {
		skill, getErr := store.GetSkill(r.Context(), *selectedSkillID)
		if getErr == sql.ErrNoRows {
			http.NotFound(w, r)
			return
		}
		if getErr != nil {
			log.Printf("get skill %d failed: %v", *selectedSkillID, getErr)
			http.Error(w, "internal server error", http.StatusInternalServerError)
			return
		}
		selectedSkill = &skill
	}

	var selectedPipeline *db.Pipeline
	var pipelineSteps []db.PipelineStep
	if selectedPipelineID != nil {
		pipeline, getErr := store.GetPipeline(r.Context(), *selectedPipelineID)
		if getErr == sql.ErrNoRows {
			http.NotFound(w, r)
			return
		}
		if getErr != nil {
			log.Printf("get pipeline %d failed: %v", *selectedPipelineID, getErr)
			http.Error(w, "internal server error", http.StatusInternalServerError)
			return
		}
		selectedPipeline = &pipeline
		steps, stepsErr := store.ListPipelineSteps(r.Context(), *selectedPipelineID)
		if stepsErr != nil {
			log.Printf("list pipeline steps for %d failed: %v", *selectedPipelineID, stepsErr)
			http.Error(w, "internal server error", http.StatusInternalServerError)
			return
		}
		pipelineSteps = steps
	}

	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	if err := pageTmpl.Execute(w, pageData{
		ActiveSection:    activeSection,
		Skills:           skills,
		SelectedSkill:    selectedSkill,
		Pipelines:        pipelines,
		SelectedPipeline: selectedPipeline,
		PipelineSteps:    pipelineSteps,
	}); err != nil {
		log.Printf("render page failed: %v", err)
		http.Error(w, "internal server error", http.StatusInternalServerError)
		return
	}
}

func parseIDPath(prefix, path string) (int64, bool) {
	if !strings.HasPrefix(path, prefix) {
		return 0, false
	}
	idPart := strings.TrimPrefix(path, prefix)
	if idPart == "" || strings.Contains(idPart, "/") {
		return 0, false
	}
	id, err := strconv.ParseInt(idPart, 10, 64)
	if err != nil {
		return 0, false
	}
	return id, true
}

func parseSkillPath(path string) (int64, string, bool) {
	parts := strings.Split(strings.TrimPrefix(path, "/skills/"), "/")
	if len(parts) != 2 {
		return 0, "", false
	}
	id, err := strconv.ParseInt(parts[0], 10, 64)
	if err != nil {
		return 0, "", false
	}
	return id, parts[1], true
}

const dbNameValidationMessage = "name must contain only lowercase letters, digits, and dashes"

func isNameValidationError(err error) bool {
	return strings.Contains(err.Error(), dbNameValidationMessage)
}

func parsePipelinePath(path string) (int64, []string, bool) {
	trimmed := strings.TrimPrefix(path, "/pipelines/")
	parts := strings.Split(trimmed, "/")
	if len(parts) < 2 {
		return 0, nil, false
	}
	pipelineID, err := strconv.ParseInt(parts[0], 10, 64)
	if err != nil {
		return 0, nil, false
	}
	for _, part := range parts[1:] {
		if part == "" {
			return 0, nil, false
		}
	}
	return pipelineID, parts[1:], true
}

func joinStepSkillNames(skills []db.PipelineStepSkill) string {
	names := make([]string, 0, len(skills))
	for _, skill := range skills {
		names = append(names, skill.Name)
	}
	return strings.Join(names, ", ")
}
