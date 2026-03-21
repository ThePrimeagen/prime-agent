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
          const nextLocation = response.headers.get("X-Skill-Location");
          if (nextLocation) {
            const base = nextLocation.endsWith("/")
              ? nextLocation.slice(0, -1)
              : nextLocation;
            form.action = `${base}/update`;
            const deleteForm = editor.querySelector("[data-skill-delete-form]");
            if (deleteForm) {
              deleteForm.action = `${base}/delete`;
            }
            const nameFromPath = decodeURIComponent(base.split("/").pop() || "");
            editor.setAttribute("data-skill-id", nameFromPath);
            history.replaceState(null, "", base);
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
