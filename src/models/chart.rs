use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ChartTaskContext {
    pub chartworkname: String,
    pub chartname: String,
    pub charturl: String,
    pub clipped_shape_folder: PathBuf,
    pub is_ifr: bool,
    pub chartdate: String,
    pub metadata_description: String,
    pub chartfolder: PathBuf,
    pub dir_1_unzipped: PathBuf,
    pub dir_2_expanded: PathBuf,
    pub dir_3_clipped: PathBuf,
    pub dir_4_mbtiles: PathBuf,
}

impl ChartTaskContext {
    pub fn new(
        workarea: &Path,
        chartworkname: String,
        chartname: String,
        charturl: String,
        clipped_shape_folder: PathBuf,
        is_ifr: bool,
        chartdate: String,
    ) -> Self {
        let metadata_description = chartname.replace('_', " ");
        let chartfolder = workarea.join(&chartname);
        let dir_1_unzipped = chartfolder.join("1_unzipped");
        let dir_2_expanded = chartfolder.join("2_expanded");
        let dir_3_clipped = chartfolder.join("3_clipped");
        let dir_4_mbtiles = chartfolder.join("4_mbtiles");

        Self {
            chartworkname,
            chartname,
            charturl,
            clipped_shape_folder,
            is_ifr,
            chartdate,
            metadata_description,
            chartfolder,
            dir_1_unzipped,
            dir_2_expanded,
            dir_3_clipped,
            dir_4_mbtiles,
        }
    }
}
