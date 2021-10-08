""" Preprocessing methods for timestamp alignment of data using pandas. """

from pandas import to_datetime

SAMPLE_INTERVAL = '50ms'
WINDOW_SIZE = '501ms'

def bucket_timestamps(timestamps, sample_interval=SAMPLE_INTERVAL):
    """ Floors a series of timestamps to some interval for easy aggregates. """
    return to_datetime(timestamps).dt.floor(sample_interval)

def max_rolling_difference(df, window_size = WINDOW_SIZE):
    """ Computes a rolling difference of points up to the window size. """
    values = df - df.rolling(window_size).min()

    timestamps = df.reset_index().timestamp.astype(int) / 10**9
    timestamps.index = df.index
    timestamps = timestamps - timestamps.rolling(window_size).min()

    return values, timestamps
