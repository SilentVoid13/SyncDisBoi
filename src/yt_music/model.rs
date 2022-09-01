use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistResponse {
    pub contents: Contents,
}
impl PlaylistResponse {
    pub fn get_mrlirs(&self) -> Option<Vec<&MusicResponsiveListItemRenderer>> {
        Some(
            self.get_section_list_contents().get(0)?
                .music_playlist_shelf_renderer
                .as_ref()?
                .contents
                .iter()
                .map(|item| &item.music_responsive_list_item_renderer)
                .collect(),
        )
    }

    pub fn get_mtrirs(&self) -> Option<Vec<&MusicTwoRowItemRenderer>> {
        Some(
            self.get_section_list_contents()
                .iter()
                .find(|item| item.item_section_renderer.is_some())?
                .item_section_renderer
                .as_ref()?
                .contents
                .iter()
                .flat_map(|item| {
                    item
                        .grid_renderer
                        .items
                        .iter()
                        .map(|item2| &item2.music_two_row_item_renderer)
                        .collect::<Vec<&MusicTwoRowItemRenderer>>()
                })
                .collect(),
        )
    }

    pub fn get_section_list_contents(&self) -> &Vec<Contents2> {
        &self.contents.single_column_browse_results_renderer.tabs[0]
            .tab_renderer
            .content
            .section_list_renderer
            .contents
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Contents {
    pub single_column_browse_results_renderer: SingleColumnBrowseResultsRenderer,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SingleColumnBrowseResultsRenderer {
    pub tabs: [Tab; 1],
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Tab {
    pub tab_renderer: TabRenderer,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TabRenderer {
    pub content: Content1,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Content1 {
    pub section_list_renderer: SectionListRenderer,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SectionListRenderer {
    pub contents: Vec<Contents2>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Contents2 {
    pub music_playlist_shelf_renderer: Option<MusicPlaylistShelfRenderer>,
    pub item_section_renderer: Option<ItemSectionRenderer>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MusicPlaylistShelfRenderer {
    pub contents: Vec<Contents3>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Contents3 {
    pub music_responsive_list_item_renderer: MusicResponsiveListItemRenderer,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MusicResponsiveListItemRenderer {
    pub menu: Option<Menu>,
    pub overlay: Option<Overlay>,
    pub flex_columns: [FlexColumn; 3],
    pub playlist_item_data: Option<PlaylistItemData>,
}
impl MusicResponsiveListItemRenderer {
    pub fn get_set_id(&self) -> Option<String> {
        Some(
            self.playlist_item_data
                .as_ref()?
                .playlist_set_video_id
                .clone(),
        )
    }

    pub fn get_id(&self) -> Option<String> {
        Some(self.playlist_item_data.as_ref()?.video_id.clone())
    }

    pub fn get_flex_run_text(&self, flex_i: usize, run_i: usize) -> Option<String> {
        Some(self.get_flex_runs(flex_i)?.get(run_i)?.get_text())
    }

    pub fn get_flex_run_id(&self, flex_i: usize, run_i: usize) -> Option<String> {
        Some(self.get_flex_runs(flex_i)?.get(run_i)?.get_id()?)
    }

    pub fn get_flex_runs(&self, flex_i: usize) -> Option<&Vec<Run>> {
        Some(
            self.flex_columns
                .get(flex_i)?
                .music_responsive_list_item_flex_column_renderer
                .text
                .runs
                .as_ref()?,
        )
    }

    fn get_action(&self) -> Option<&Action> {
        Some(
            &self
                .menu
                .as_ref()?
                .menu_renderer
                .items
                .iter()
                .filter_map(|item| {
                    Some(&item.menu_service_item_renderer.as_ref()?.service_endpoint)
                })
                .find(|item| item.playlist_edit_endpoint.is_some())?
                .playlist_edit_endpoint
                .as_ref()?
                .actions[0],
        )
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ItemSectionRenderer {
    pub contents: Vec<Contents4>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Contents4 {
    pub grid_renderer: GridRenderer,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GridRenderer {
    pub items: Vec<Item2>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Item2 {
    pub music_two_row_item_renderer: MusicTwoRowItemRenderer,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MusicTwoRowItemRenderer {
    pub menu: Option<Menu>,
    pub title: Text,
}
impl MusicTwoRowItemRenderer {
    pub fn get_id(&self) -> Option<String> {
        Some(self.title.runs.as_ref()?.get(0)?.get_id()?)
    }

    pub fn get_name(&self) -> Option<String> {
        Some(self.title.runs.as_ref()?.get(0)?.get_text())
    }
}



#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FlexColumn {
    pub music_responsive_list_item_flex_column_renderer: MusicResponsiveListItemFlexColumnRenderer,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MusicResponsiveListItemFlexColumnRenderer {
    pub text: Text,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Text {
    pub runs: Option<Vec<Run>>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub text: String,
    pub navigation_endpoint: Option<NavigationEndpoint>,
}
impl Run {
    pub fn get_text(&self) -> String {
        self.text.clone()
    }

    pub fn get_id(&self) -> Option<String> {
        Some(
            self.navigation_endpoint
                .as_ref()?
                .browse_endpoint
                .as_ref()?
                .browse_id
                .clone(),
        )
    }
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NavigationEndpoint {
    pub browse_endpoint: Option<BrowseEndpoint>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BrowseEndpoint {
    pub browse_id: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Menu {
    pub menu_renderer: MenuRenderer,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MenuRenderer {
    pub items: Vec<Item>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub menu_service_item_renderer: Option<MenuServiceItemRenderer>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MenuServiceItemRenderer {
    pub service_endpoint: ServiceEndpoint,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServiceEndpoint {
    pub playlist_edit_endpoint: Option<PlaylistEditEndpoint>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistEditEndpoint {
    pub actions: [Action; 1],
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    pub action: String,
    pub removed_video_id: String,
    pub set_video_id: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Overlay {
    pub music_item_thumbnail_overlay_renderer: MusicItemThumbnailOverlayRenderer,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MusicItemThumbnailOverlayRenderer {
    pub content: Content2,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Content2 {
    pub music_play_button_renderer: MusicPlayButtonRenderer,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MusicPlayButtonRenderer {
    pub play_navigation_endpoint: Option<PlayNavigationEndpoint>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlayNavigationEndpoint {
    pub watch_endpoint: WatchEndpoint,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WatchEndpoint {
    pub playlist_id: String,
    pub video_id: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistItemData {
    pub playlist_set_video_id: String,
    pub video_id: String,
}
