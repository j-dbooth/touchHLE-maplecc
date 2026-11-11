/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
package org.touchhle.android;

import android.content.Context;
import android.content.pm.ApplicationInfo;
import android.database.Cursor;
import android.database.MatrixCursor;
import android.os.CancellationSignal;
import android.os.ParcelFileDescriptor;
import android.provider.DocumentsContract;
import android.webkit.MimeTypeMap;

import java.io.File;
import java.io.FileInputStream;
import java.io.FileNotFoundException;
import java.io.FileOutputStream;
import java.io.IOException;

/**
 * Storage DocumentsProvider enabling system file managers and modding tools
 * to access MapleStory: Cave Crawlers user data on Android.
 */
public class DocumentsProvider extends android.provider.DocumentsProvider {
    private static final String ROOT_ID = "root";

    private static final String[] DEFAULT_ROOT_PROJECTION = new String[] {
        DocumentsContract.Root.COLUMN_ROOT_ID,
        DocumentsContract.Root.COLUMN_MIME_TYPES,
        DocumentsContract.Root.COLUMN_FLAGS,
        DocumentsContract.Root.COLUMN_ICON,
        DocumentsContract.Root.COLUMN_TITLE,
        DocumentsContract.Root.COLUMN_SUMMARY,
        DocumentsContract.Root.COLUMN_DOCUMENT_ID,
        DocumentsContract.Root.COLUMN_AVAILABLE_BYTES
    };

    private static final String[] DEFAULT_DOCUMENT_PROJECTION = new String[] {
        DocumentsContract.Document.COLUMN_DOCUMENT_ID,
        DocumentsContract.Document.COLUMN_MIME_TYPE,
        DocumentsContract.Document.COLUMN_DISPLAY_NAME,
        DocumentsContract.Document.COLUMN_LAST_MODIFIED,
        DocumentsContract.Document.COLUMN_FLAGS,
        DocumentsContract.Document.COLUMN_SIZE
    };

    private File getBaseDirectory() {
        Context context = getContext();
        if (context == null) {
            throw new IllegalStateException("Context is null");
        }
        File dir = context.getExternalFilesDir(null);
        if (dir == null) {
            throw new IllegalStateException("ExternalFilesDir is null");
        }
        return dir;
    }

    private String getApplicationName() {
        Context context = getContext();
        if (context != null) {
            ApplicationInfo ai = context.getApplicationInfo();
            if (ai != null) {
                CharSequence label = context.getPackageManager().getApplicationLabel(ai);
                if (label != null) {
                    return label.toString();
                }
            }
        }
        return "MapleStory: Cave Crawlers";
    }

    private int getApplicationIcon() {
        Context context = getContext();
        if (context != null) {
            ApplicationInfo ai = context.getApplicationInfo();
            if (ai != null && ai.icon != 0) {
                return ai.icon;
            }
        }
        return android.R.drawable.sym_def_app_icon;
    }

    @Override
    public boolean onCreate() {
        return true;
    }

    private File getFile(String documentId) throws FileNotFoundException {
        if (documentId != null && documentId.startsWith(ROOT_ID)) {
            File base = getBaseDirectory();
            if (documentId.length() == ROOT_ID.length()) {
                return base;
            }
            String relPath = documentId.substring(ROOT_ID.length() + 1);
            File file = new File(base, relPath);
            if (!file.exists()) {
                throw new FileNotFoundException(file.getAbsolutePath() + " (" + documentId + ") not found");
            }
            return file;
        }
        throw new FileNotFoundException("'" + documentId + "' is not in any known root");
    }

    private String getDocumentId(File file) {
        File base = getBaseDirectory();
        String basePath = base.getAbsolutePath();
        String filePath = file.getAbsolutePath();
        if (filePath.equals(basePath)) {
            return ROOT_ID;
        }
        if (filePath.startsWith(basePath + File.separator)) {
            String rel = filePath.substring(basePath.length() + 1).replace(File.separatorChar, '/');
            return ROOT_ID + "/" + rel;
        }
        return ROOT_ID + "/" + file.getName();
    }

    @Override
    public Cursor queryRoots(String[] projection) {
        MatrixCursor cursor = new MatrixCursor(projection != null ? projection : DEFAULT_ROOT_PROJECTION);
        File base = getBaseDirectory();
        cursor.newRow()
            .add(DocumentsContract.Root.COLUMN_ROOT_ID, ROOT_ID)
            .add(DocumentsContract.Root.COLUMN_SUMMARY, null)
            .add(DocumentsContract.Root.COLUMN_FLAGS,
                DocumentsContract.Root.FLAG_SUPPORTS_CREATE | DocumentsContract.Root.FLAG_SUPPORTS_IS_CHILD)
            .add(DocumentsContract.Root.COLUMN_TITLE, getApplicationName())
            .add(DocumentsContract.Root.COLUMN_DOCUMENT_ID, getDocumentId(base))
            .add(DocumentsContract.Root.COLUMN_MIME_TYPES, "*/*")
            .add(DocumentsContract.Root.COLUMN_AVAILABLE_BYTES, base.getFreeSpace())
            .add(DocumentsContract.Root.COLUMN_ICON, getApplicationIcon());
        return cursor;
    }

    @Override
    public Cursor queryDocument(String documentId, String[] projection) throws FileNotFoundException {
        MatrixCursor cursor = new MatrixCursor(projection != null ? projection : DEFAULT_DOCUMENT_PROJECTION);
        includeFile(cursor, documentId, null);
        return cursor;
    }

    @Override
    public boolean isChildDocument(String parentDocumentId, String documentId) {
        return documentId != null && parentDocumentId != null && documentId.startsWith(parentDocumentId);
    }

    private File resolveWithoutConflict(File parent, String name) {
        File file = new File(parent, name);
        if (file.exists()) {
            int noConflictId = 1;
            int lastDot = name.lastIndexOf('.');
            String baseName = lastDot > 0 ? name.substring(0, lastDot) : name;
            String ext = lastDot > 0 ? name.substring(lastDot) : "";
            while (file.exists()) {
                file = new File(parent, baseName + " (" + (noConflictId++) + ")" + ext);
            }
        }
        return file;
    }

    @Override
    public String createDocument(String parentDocumentId, String mimeType, String displayName) throws FileNotFoundException {
        File parentFile = getFile(parentDocumentId);
        File newFile = resolveWithoutConflict(parentFile, displayName);
        try {
            if (DocumentsContract.Document.MIME_TYPE_DIR.equals(mimeType)) {
                if (!newFile.mkdir()) {
                    throw new IOException("Failed to create directory");
                }
            } else {
                if (!newFile.createNewFile()) {
                    throw new IOException("Failed to create file");
                }
            }
        } catch (IOException e) {
            throw new FileNotFoundException("Couldn't create document '" + newFile.getPath() + "': " + e.getMessage());
        }
        return getDocumentId(newFile);
    }

    @Override
    public void deleteDocument(String documentId) throws FileNotFoundException {
        File file = getFile(documentId);
        if (!file.delete()) {
            throw new FileNotFoundException("Couldn't delete document with ID '" + documentId + "'");
        }
    }

    @Override
    public void removeDocument(String documentId, String parentDocumentId) throws FileNotFoundException {
        File parent = getFile(parentDocumentId);
        File file = getFile(documentId);
        File fileParent = file.getParentFile();
        if (parent.equals(file) || fileParent == null || fileParent.equals(parent)) {
            if (!file.delete()) {
                throw new FileNotFoundException("Couldn't delete document with ID '" + documentId + "'");
            }
        } else {
            throw new FileNotFoundException("Couldn't delete document with ID '" + documentId + "'");
        }
    }

    @Override
    public String renameDocument(String documentId, String displayName) throws FileNotFoundException {
        if (displayName == null) {
            throw new FileNotFoundException("Couldn't rename document '" + documentId + "' as the new name is null");
        }
        File sourceFile = getFile(documentId);
        File sourceParentFile = sourceFile.getParentFile();
        if (sourceParentFile == null) {
            throw new FileNotFoundException("Couldn't rename document '" + documentId + "' as it has no parent");
        }
        File destFile = new File(sourceParentFile, displayName);
        try {
            if (!sourceFile.renameTo(destFile)) {
                throw new FileNotFoundException("Couldn't rename document from '" + sourceFile.getName() + "' to '" + destFile.getName() + "'");
            }
        } catch (Exception e) {
            throw new FileNotFoundException("Couldn't rename document from '" + sourceFile.getName() + "' to '" + destFile.getName() + "': " + e.getMessage());
        }
        return getDocumentId(destFile);
    }

    @Override
    public String copyDocument(String sourceDocumentId, String targetParentDocumentId) throws FileNotFoundException {
        File parent = getFile(targetParentDocumentId);
        File oldFile = getFile(sourceDocumentId);
        File newFile = resolveWithoutConflict(parent, oldFile.getName());
        try {
            if (!newFile.createNewFile()) {
                throw new IOException("Couldn't create new file");
            }
            try (FileInputStream in = new FileInputStream(oldFile);
                 FileOutputStream out = new FileOutputStream(newFile)) {
                byte[] buffer = new byte[65536];
                int len;
                while ((len = in.read(buffer)) > 0) {
                    out.write(buffer, 0, len);
                }
            }
        } catch (IOException e) {
            throw new FileNotFoundException("Couldn't copy document '" + sourceDocumentId + "': " + e.getMessage());
        }
        return getDocumentId(newFile);
    }

    @Override
    public String moveDocument(String sourceDocumentId, String sourceParentDocumentId, String targetParentDocumentId) throws FileNotFoundException {
        String newDocumentId = copyDocument(sourceDocumentId, targetParentDocumentId);
        removeDocument(sourceDocumentId, sourceParentDocumentId);
        return newDocumentId;
    }

    private void includeFile(MatrixCursor cursor, String documentId, File file) throws FileNotFoundException {
        String localDocumentId = documentId != null ? documentId : getDocumentId(file);
        File localFile = file != null ? file : getFile(documentId);

        int flags = 0;
        if (localFile.isDirectory() && localFile.canWrite()) {
            flags = DocumentsContract.Document.FLAG_DIR_SUPPORTS_CREATE;
        } else if (localFile.canWrite()) {
            flags = DocumentsContract.Document.FLAG_SUPPORTS_WRITE
                    | DocumentsContract.Document.FLAG_SUPPORTS_DELETE
                    | DocumentsContract.Document.FLAG_SUPPORTS_REMOVE
                    | DocumentsContract.Document.FLAG_SUPPORTS_MOVE
                    | DocumentsContract.Document.FLAG_SUPPORTS_COPY
                    | DocumentsContract.Document.FLAG_SUPPORTS_RENAME;
        }

        File base = getBaseDirectory();
        String displayName = localFile.equals(base) ? getApplicationName() : localFile.getName();

        cursor.newRow()
            .add(DocumentsContract.Document.COLUMN_DOCUMENT_ID, localDocumentId)
            .add(DocumentsContract.Document.COLUMN_DISPLAY_NAME, displayName)
            .add(DocumentsContract.Document.COLUMN_SIZE, localFile.length())
            .add(DocumentsContract.Document.COLUMN_MIME_TYPE, getTypeForFile(localFile))
            .add(DocumentsContract.Document.COLUMN_LAST_MODIFIED, localFile.lastModified())
            .add(DocumentsContract.Document.COLUMN_FLAGS, flags);
    }

    private String getTypeForFile(File file) {
        if (file.isDirectory()) {
            return DocumentsContract.Document.MIME_TYPE_DIR;
        }
        return getTypeForName(file.getName());
    }

    private String getTypeForName(String name) {
        int lastDot = name.lastIndexOf('.');
        if (lastDot >= 0) {
            String ext = name.substring(lastDot + 1);
            String mime = MimeTypeMap.getSingleton().getMimeTypeFromExtension(ext);
            if (mime != null) {
                return mime;
            }
        }
        return "application/octet-stream";
    }

    @Override
    public Cursor queryChildDocuments(String parentDocumentId, String[] projection, String sortOrder) throws FileNotFoundException {
        MatrixCursor cursor = new MatrixCursor(projection != null ? projection : DEFAULT_DOCUMENT_PROJECTION);
        File parent = getFile(parentDocumentId);
        File[] children = parent.listFiles();
        if (children != null) {
            for (File file : children) {
                includeFile(cursor, null, file);
            }
        }
        return cursor;
    }

    @Override
    public ParcelFileDescriptor openDocument(String documentId, String mode, CancellationSignal signal) throws FileNotFoundException {
        File file = getFile(documentId);
        int accessMode = ParcelFileDescriptor.parseMode(mode);
        return ParcelFileDescriptor.open(file, accessMode);
    }
}
