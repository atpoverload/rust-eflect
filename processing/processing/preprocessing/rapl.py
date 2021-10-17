""" Processing for rapl data using pandas. """

import pandas as pd

from processing.preprocessing.util import bucket_timestamps
from processing.preprocessing.util import max_rolling_difference

WRAP_AROUND_VALUE = 16384

def parse_rapl_samples(samples):
    """ Converts a collection of RaplSamples to a DataFrame. """
    records = []
    for sample in samples:
        for reading in sample.reading:
            records.append([
                sample.timestamp,
                reading.socket,
                reading.cpu,
                reading.package,
                reading.dram,
                reading.gpu
            ])
    df = pd.DataFrame(records)
    df.columns = [
        'timestamp',
        'socket',
        'cpu',
        'package',
        'dram',
        'gpu'
    ]
    df.timestamp = pd.to_datetime(df.timestamp, unit='ms')
    return df

# TODO(timur): i've been told by alejandro that the value i'm using isn't
#   actually the wrap around. i'm not sure how to look it up properly however.
def maybe_apply_wrap_around(value):
    """ Checks if the value needs to be adjusted by the wrap around. """
    if value < 0:
        return value + WRAP_AROUND_VALUE
    else:
        return value

def process_rapl_data(df):
    """ Computes the power of each 50ms bucket """
    df.timestamp = bucket_timestamps(df.timestamp)
    df = df.groupby(['timestamp', 'socket']).min()
    df.columns.name = 'component'

    energy, ts = max_rolling_difference(df.unstack())
    energy = energy.stack().stack().apply(maybe_apply_wrap_around)
    energy = energy.groupby(['timestamp', 'socket', 'component']).sum().div(ts, axis = 0)

    return energy

def rapl_samples_to_df(samples):
    """ Converts a collection of RaplSamples to a processed DataFrame. """
    return process_rapl_data(parse_rapl_samples(samples))
