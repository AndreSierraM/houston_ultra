import { useCallback, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { useQueryClient } from "@tanstack/react-query";
import { FilesBrowser } from "@houston-ai/agent";
import {
  EmptyDescription,
  EmptyHeader,
  EmptyTitle,
  Spinner,
} from "@houston-ai/core";
import { FolderOpen } from "lucide-react";
import {
  useFiles,
  useDeleteFile,
  useRenameFile,
  useCreateFolder,
} from "../../hooks/queries";
import { engineAgentPath } from "../../lib/engine-agent-path";
import { queryKeys } from "../../lib/query-keys";
import { tauriFiles } from "../../lib/tauri";
import { isCloudAgent } from "../../lib/runtime-router";
import { useUIStore } from "../../stores/ui";
import type { TabProps } from "../../lib/types";

function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = reader.result;
      if (typeof result !== "string") {
        reject(new Error("Failed to read file"));
        return;
      }
      const comma = result.indexOf(",");
      resolve(comma >= 0 ? result.slice(comma + 1) : result);
    };
    reader.onerror = () => reject(reader.error ?? new Error("Failed to read file"));
    reader.readAsDataURL(file);
  });
}

export default function FilesTab({ agent }: TabProps) {
  const { t } = useTranslation("agents");
  const addToast = useUIStore((s) => s.addToast);
  const qc = useQueryClient();
  const fileInputRef = useRef<HTMLInputElement>(null);

  const enginePath = engineAgentPath(agent);
  const cloud = isCloudAgent(agent);

  const { data: files, isLoading: loading, isError, error } = useFiles(agent);
  const deleteFile = useDeleteFile(agent);
  const renameFile = useRenameFile(agent);
  const createFolder = useCreateFolder(agent);

  useEffect(() => {
    if (!isError || !error) return;
    addToast({
      title: t("files.loadError"),
      description: errorMessage(error),
      variant: "error",
    });
  }, [isError, error, addToast, t]);

  const importFiles = useCallback(
    async (picked: File[], targetFolder?: string) => {
      if (!enginePath) return;
      for (const file of picked) {
        const dataBase64 = await fileToBase64(file);
        const fileName = targetFolder ? `${targetFolder}/${file.name}` : file.name;
        await tauriFiles.importBytes(enginePath, fileName, dataBase64, agent);
      }
      await qc.invalidateQueries({ queryKey: queryKeys.files(enginePath) });
    },
    [agent, enginePath, qc],
  );

  const handleFilesDropped = useCallback(
    (dropped: File[], targetFolder?: string) => {
      void importFiles(dropped, targetFolder).catch(() => {
        // importBytes surfaces failures via call()
      });
    },
    [importFiles],
  );

  const handleBrowse = useCallback(() => {
    fileInputRef.current?.click();
  }, []);

  const handleFileInputChange = useCallback(
    (event: React.ChangeEvent<HTMLInputElement>) => {
      const picked = event.target.files;
      if (!picked?.length) return;
      void importFiles(Array.from(picked)).catch(() => {
        // importBytes surfaces failures via call()
      });
      event.target.value = "";
    },
    [importFiles],
  );

  const handleCloudOpen = useCallback(() => {
    addToast({
      title: t("files.cloudOpenUnavailable"),
      variant: "error",
    });
  }, [addToast, t]);

  const browserLabels = {
    columnName: t("files.columns.name"),
    columnDateModified: t("files.columns.dateModified"),
    columnSize: t("files.columns.size"),
    columnKind: t("files.columns.kind"),
    loading: t("files.loading"),
    browseFiles: t("files.browseFiles"),
  };
  const menuLabels = {
    open: t("files.menu.open"),
    rename: t("files.menu.rename"),
    reveal: t("files.menu.reveal"),
    delete: t("files.menu.delete"),
  };

  if (!enginePath) {
    return (
      <div className="h-full overflow-hidden p-4 flex flex-col items-center pt-[20vh] gap-4 px-8">
        <Spinner className="size-8" />
        <EmptyHeader>
          <EmptyTitle>{t("files.provisioningTitle")}</EmptyTitle>
          <EmptyDescription>{t("files.provisioningDescription")}</EmptyDescription>
        </EmptyHeader>
      </div>
    );
  }

  return (
    <div className="h-full overflow-hidden p-4">
      <input
        ref={fileInputRef}
        type="file"
        multiple
        className="hidden"
        onChange={handleFileInputChange}
      />
      <FilesBrowser
        files={files ?? []}
        loading={loading}
        onOpen={
          cloud
            ? () => {
                handleCloudOpen();
              }
            : (file) => tauriFiles.open(enginePath, file.path)
        }
        onReveal={cloud ? undefined : (file) => tauriFiles.reveal(enginePath, file.path)}
        onDelete={(file) => deleteFile.mutate(file.path)}
        onRename={(file, newName) => renameFile.mutate({ relativePath: file.path, newName })}
        onCreateFolder={(name) => createFolder.mutate(name)}
        onFilesDropped={handleFilesDropped}
        onBrowse={handleBrowse}
        emptyTitle={t("files.emptyTitle")}
        emptyDescription={t("files.emptyDescription")}
        labels={browserLabels}
        menuLabels={menuLabels}
        statusBarAction={
          cloud ? undefined : (
            <button
              onClick={() => tauriFiles.revealAgent(enginePath)}
              className="flex items-center gap-1 text-[11px] text-[#6d6d6d] hover:text-[#0d0d0d] transition-colors"
            >
              <FolderOpen className="size-3" />
              {t("files.openInFileManager")}
            </button>
          )
        }
      />
    </div>
  );
}
