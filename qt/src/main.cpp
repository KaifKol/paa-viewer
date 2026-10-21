#include <QApplication>
#include <QStyleFactory>
#include "mainwindow.h"

int main(int argc, char* argv[]) {
    QApplication app(argc, argv);
    app.setApplicationName("PAA Viewer");
    app.setApplicationDisplayName("PAA Viewer & Converter (Qt6)");
    app.setOrganizationName("omp");

    // Let Qt pick the native desktop theme and palette from the OS (KDE Breeze, GNOME, etc.)

    QString initialFile;
    if (argc > 1) {
        initialFile = QString::fromLocal8Bit(argv[1]);
    }

    MainWindow w(initialFile);
    w.show();

    return app.exec();
}
